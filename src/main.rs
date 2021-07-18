#[macro_use]
extern crate rocket;

pub mod id_gen;

use rand::seq::IteratorRandom;
use rocket::response::NamedFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use ws_hotel::{
    Context, Message, Relocation, ResultRelocation, Room, RoomRef, RoomHandler, CloseCode,
    RoomRefWeak,
};
use crate::id_gen::FunnyWords;
use std::sync::{Arc, Weak};

#[derive(Clone, Debug, Serialize)]
struct Player {
    username: String,
    avatar: String,
}

#[derive(Debug, Deserialize)]
enum Prompt {
    Question(String),
    Tag(String, Vec<(String, String, String)>),
}

#[derive(Serialize)]
#[serde(tag = "tag", content = "prompt")]
enum ClientPrompt<'a> {
    Question(&'a str),
    Tag(&'a str, &'a str, Vec<(&'a str, &'a str, &'a str)>),
}

#[derive(Serialize)]
enum ErrorMsg {
    UsedUsername,
    RoomNotFound,
}

// For a question : First username = Player who voted, second username = Choice
// For a Tag : First username = Player who voted, second username = Choice
type Vote = (Username, Username);
type Username = String;

#[derive(Debug)]
struct GameRoom {
    lobby: RoomRef<Lobby>,
    all_questions: Weak<[Prompt]>,

    code: String,
    players: Vec<Player>,
    votes: Vec<Vote>,
    questions_count: u32,
    questions: Vec<usize>,
}

#[derive(Serialize)]
#[serde(tag = "tag")]
enum ServerEvent<'a> {
    /// Reply to a [ClientEventLobby::RoomProbe]
    ///
    /// If [`code`][ServerEvent::RoomProbeResult::code]...
    ///   * ... is [None], the room doesn't exist
    ///   * ... is [Some], the room exists, and its code is given back
    RoomProbeResult {
        code: Option<&'a str>,
    },

    /// When Server sends all the infos about the room to a Client that has just joined
    OnRoomJoin {
        code: &'a str,
        players: Vec<Player>,
        question_counter: u32,
        // TODO règles
    },

    /// When Server tells Clients that something has changed in the room (e.g. new player)
    RoomUpdate {
        players: Vec<Player>,
    },
    /// When Server tells Clients that a new Round has started (May also start the game)
    NewRound {
        question: ClientPrompt<'a>,
    },
    //Server tells Clients that something has changed in the round (e.g. X players have responded)
    RoundUpdate {
        ready_player_count: u32,
    },
    /// When Server tells Clients that the round is over (the time is over / everyone responded)
    RoundOver {
        votes: Vec<Vote>,
    },
    Error {
        code: ErrorMsg,
    },
    GameOver,
}

#[derive(Deserialize)]
#[serde(tag = "tag")]
enum ClientEventLobby {
    /// Asks the server if a room exists
    ///
    /// The server must always reply with a [ServerEvent::RoomProbeResult]
    RoomProbe {
        code: String,
    },

    /// Asks the server to join a room if a code is given, or to create one and join it immediately
    /// else.
    JoinRoom {
        username: Username,
        avatar: String,
        code: Option<String>,
        // TODO règles (replace this enum field with another enum)
    },
}

#[derive(Deserialize)]
#[serde(tag = "tag")]
enum ClientEventGame {
    /// When the admin of the room starts the game
    StartRound,
    /// When Client answers to a question
    Answer { vote: Vote },

    /// Sent by a client who wants to leave its current game room. May be sent at any point in time,
    /// but, as of today, a normal client should only display a "leave" button at the end of a game.
    LeaveRoom,
}

impl From<&ServerEvent<'_>> for Message {
    fn from(event: &ServerEvent<'_>) -> Self {
        Message::Text(serde_json::to_string(event).unwrap())
    }
}

#[derive(Debug)]
struct Lobby {
    funny_words: FunnyWords,
    questions: Arc<[Prompt]>,
    rooms: HashMap<String, RoomRefWeak<GameRoom>>,
}

impl RoomHandler for Lobby {
    type Guest = ();

    fn on_message(&mut self, cx: Context<Self>, msg: Message) -> ResultRelocation {
        use ServerEvent::*;

        let msg = match msg {
            Message::Text(s) => s,
            _ => unreachable!(),
        };

        let evt = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            _ => return Ok(None),
        };

        match evt {
            ClientEventLobby::RoomProbe { code } => {
                let code = code.as_str();

                let code = self.rooms.get_mut(code).and_then(|weak| weak.upgrade()).map(|_| code);

                cx.send(&ServerEvent::RoomProbeResult {
                    code,
                })?;

                Ok(None)
            },
            ClientEventLobby::JoinRoom {
                username,
                avatar,
                code,
            } => {
                let room = if let Some(code) = code {
                    // Get the room of given code
                    match self.rooms.get_mut(&code).and_then(|r| r.upgrade()) {
                        Some(room) => room,
                        None => {
                            cx.send(&Error {
                                code: ErrorMsg::RoomNotFound,
                            })?;

                            return Ok(None);
                        }
                    }
                } else {
                    let code = loop {
                        let chain =
                            id_gen::Chain::new(&self.funny_words, 0.1, rand::thread_rng());

                        let code = chain.take(8).collect::<String>();

                        if !self.rooms.contains_key(&code) {
                            break code;
                        }
                    };

                    let mut rng = rand::thread_rng();
                    let room = GameRoom::create(
                        cx.room().upgrade().unwrap(),
                        Arc::downgrade(&self.questions),
                        code, &self.questions,
                        &mut rng
                    );

                    let code = room.code.clone();

                    let room_ref = Room::new(room);
                    self.rooms.insert(code, room_ref.downgrade());

                    room_ref
                };

                // Ensure the username is not already used
                let already_used = room.with(|r| {
                    if r.players.iter().any(|x| x.username == username) {
                        Err(Error {
                            code: ErrorMsg::UsedUsername,
                        })
                    } else {
                        // Add the player to the room
                        r.join(Player {
                            username: username.clone(),
                            avatar,
                        });

                        Ok(())
                    }
                });

                if let Err(err) = already_used {
                    cx.send(&err)?;
                    return Ok(None);
                }

                let relocation = Relocation::new(&room, username.clone());

                println!("Room = {:?}", room);

                Ok(Some(relocation))
            }
        }
    }
}

impl RoomHandler for GameRoom {
    type Guest = String;

    fn on_join(&mut self, cx: Context<Self>) -> ResultRelocation {
        // Send an update to each other player
        cx.broadcast(&ServerEvent::RoomUpdate {
            players: self.players.clone(),
        })?;

        cx.send(&ServerEvent::OnRoomJoin {
            code: &self.code,
            players: self.players.clone(),
            question_counter: self.questions_count,
        })?;

        Ok(None)
    }

    fn on_message(&mut self, cx: Context<Self>, msg: Message) -> ResultRelocation {
        use ServerEvent::*;

        let msg = match msg {
            Message::Text(s) => s,
            _ => unreachable!(),
        };

        let evt = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            _ => return Ok(None),
        };

        match evt {
            ClientEventGame::StartRound => {
                self.votes.clear();
                if self.questions_count > 9 {
                    cx.broadcast(&GameOver)?;

                    Ok(None)
                } else {
                    let question = &self.all_questions.upgrade().unwrap()[self.questions[self.questions_count as usize]];
                    self.questions_count += 1;

                    use rand::seq::SliceRandom;
                    let mut rng = rand::thread_rng();
                    let mut players_rand = self.players.clone();
                    players_rand.shuffle(&mut rng);
                    let mut players_rand = players_rand.into_iter();

                    cx.broadcast_with(|_| {
                        Message::from(&NewRound {
                            question: question.into_client(&players_rand.next().unwrap().username),
                        })
                    })?;

                    Ok(None)
                }
            }
            ClientEventGame::Answer { vote } => {
                self.record_vote(vote);
                let res = if self.votes.len() < self.players.len() {
                    RoundUpdate {
                        ready_player_count: self.votes.len() as u32,
                    }
                } else {
                    RoundOver {
                        votes: self.votes.clone(),
                    }
                };

                cx.broadcast(&res)?;

                Ok(None)
            }
            ClientEventGame::LeaveRoom => {
                Ok(Some(Relocation::new(&self.lobby, ())))
            }
        }
    }

    fn on_leave(&mut self, mut cx: Context<Self>, _code_and_reason: Option<(CloseCode, &str)>) {
        let me = cx.identity().as_str();

        self.votes.retain(|(voter, _)| voter != me);
        self.players.retain(|p| p.username != me);

        let _ignored_error = cx.broadcast(&ServerEvent::RoomUpdate {
            players: self.players.clone(),
        });
    }
}

#[rocket::launch]
fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .mount("/", routes![index,])
        .mount(
            "/static",
            rocket_contrib::serve::StaticFiles::from("static"),
        )
        .attach(rocket::fairing::AdHoc::on_launch("WebSocket", |_| {
            let funny_words = {
                let file = BufReader::new(
                    std::fs::File::open("funny_words.txt").expect("missing funny_words.txt"),
                );

                file.lines()
                        .filter_map(|l| l.ok())
                        .filter(|line| !line.is_empty())
                        .collect::<FunnyWords>()
            };

            let questions = {
                // TODO: c pa opti
                let content = std::fs::read_to_string("questions.ron").expect("Error while reading question.ron file");
                let questions: Vec<Prompt> = ron::from_str(&content).expect("Error while parsing questions");

                println!("Read {} questions ({} tags)", questions.len(), questions.iter().filter(|x| match x {
                    Prompt::Tag(_, _) => true,
                    _ => false,
                }).count());

                questions.into_boxed_slice().into()
            };

            std::thread::spawn(move || {
                ws_hotel::listen(
                    "0.0.0.0:8008",
                    Lobby {
                        funny_words,
                        questions,
                        rooms: HashMap::new(),
                    },
                );
            });
        }))
}

#[get("/")]
async fn index() -> Option<NamedFile> {
    NamedFile::open("static/index.html").await.ok()
}

impl GameRoom {
    fn create<R: rand::Rng + ?Sized>(
        lobby: RoomRef<Lobby>,
        all_questions: Weak<[Prompt]>,
        code: String,
        questions: &[Prompt],
        rng: &mut R
    ) -> Self {
        // Randomly choose 10 index for questions
        let v = (0..questions.len()).choose_multiple(rng, 10);
        Self {
            lobby,
            all_questions,
            code,
            players: vec![],
            votes: vec![],
            questions_count: 0,
            questions: v,
        }
    }

    fn join(&mut self, player: Player) {
        self.players.push(player);
    }

    fn record_vote(&mut self, vote: Vote) {
        if !self.votes.iter().any(|x| x.0 == vote.0) {
            self.votes.push(vote);
        }
    }
}

impl Prompt {
    fn into_client<'a>(&'a self, username: &'a str) -> ClientPrompt<'a> {
        match self {
            Prompt::Question(s) => ClientPrompt::Question(&s),
            Prompt::Tag(s, v) => ClientPrompt::Tag(
                &s,
                username,
                v.iter()
                    .map(|x| (x.0.as_str(), x.1.as_str(), x.2.as_str()))
                    .collect(),
            ),
        }
    }
}
