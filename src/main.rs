#[macro_use]
extern crate rocket;

pub mod id_gen;

use rand::seq::IteratorRandom;
use rocket::response::NamedFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Mutex;
use ws_hotel::{Message, ws};

#[derive(Clone, Debug, Serialize)]
struct Player {
    username: String,
    avatar: String,
    #[serde(skip)]
    ws: ws::Sender,
}

#[derive(Deserialize)]
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
    code: String,
    players: Vec<Player>,
    votes: Vec<Vote>,
    questions_count: u32,
    questions: Vec<usize>,
}

#[derive(Serialize)]
#[serde(tag = "tag")]
enum ServerEvent<'a> {
    /// When Server created a room and tells Client that the room is ready
    RoomCreated {
        code: String,
    },
    /// When Server sends all the infos about the room to a Client that has just joined
    OnRoomJoin {
        players: Vec<Player>,
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
enum ClientEvent {
    /***** Client *****/
    /// Client ask Server to create a room
    CreateRoom, //TODO règles
    /// When Client joins a room
    JoinRoom {
        username: Username,
        avatar: String,
        code: String,
    },
    /// When the admin of the room starts the game
    StartRound { code: String },
    /// When Client answers to a question
    Answer { code: String, vote: Vote },
}

lazy_static::lazy_static! {
    static ref ROOMS: Mutex<HashMap<String, GameRoom>> = Mutex::new(HashMap::new());
    static ref QUESTIONS: Vec<Prompt> = {
        // TODO: c pa opti
        let content = std::fs::read_to_string("questions.ron").expect("Error while reading question.ron file");
        let questions: Vec<Prompt> = ron::from_str(&content).expect("Error while parsing questions");

        println!("Read {} questions ({} tags)", questions.len(), questions.iter().filter(|x| match x {
            Prompt::Tag(_, _) => true,
            _ => false,
        }).count());

        questions
    };
}

impl From<&ServerEvent<'_>> for Message {
    fn from(event: &ServerEvent<'_>) -> Self {
        Message::Text(serde_json::to_string(event).unwrap())
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
            let funny_words: &'static _ = {
                let file = BufReader::new(
                    std::fs::File::open("funny_words.txt").expect("missing funny_words.txt"),
                );

                let funny_words: Box<id_gen::FunnyWords> = Box::new(
                    file.lines()
                        .filter_map(|l| l.ok())
                        .filter(|line| !line.is_empty())
                        .collect(),
                );

                Box::leak(funny_words)
            };

            std::thread::spawn(move || {
                ws::listen("0.0.0.0:8008", |out| {
                    move |msg| {
                        use ClientEvent::*;
                        use ServerEvent::*;

                        let msg = match msg {
                            Message::Text(s) => s,
                            _ => unreachable!(),
                        };

                        let evt: ClientEvent = match serde_json::from_str(&msg) {
                            Ok(msg) => msg,
                            _ => return Ok(()),
                        };

                        match evt {
                            CreateRoom => {
                                let mut rooms = ROOMS.lock().unwrap();

                                let code = loop {
                                    let chain =
                                        id_gen::Chain::new(funny_words, 0.1, rand::thread_rng());

                                    let code = chain.take(8).collect::<String>();

                                    if !rooms.contains_key(&code) {
                                        break code;
                                    }
                                };

                                let mut rng = rand::thread_rng();
                                let room = GameRoom::create(code, &QUESTIONS, &mut rng);

                                let res = RoomCreated {
                                    code: room.code.clone(),
                                };

                                rooms.insert(room.code.clone(), room);

                                out.send(&res)?;
                            }
                            JoinRoom {
                                username,
                                avatar,
                                code,
                            } => {
                                // Get the room of given code
                                let mut rooms = ROOMS.lock().unwrap();
                                let room = match rooms.get_mut(&code) {
                                    Some(room) => room,
                                    None => return out.send(&Error {
                                        code: ErrorMsg::RoomNotFound,
                                    }),
                                };

                                // Ensure the username is not already used
                                if room.players.iter().any(|x| x.username == username) {
                                    out.send(&Error {
                                        code: ErrorMsg::UsedUsername,
                                    })?;

                                    return Ok(());
                                }
                                // Fetch the websocket Sender element for each player, in order to send a RoomUpdate event
                                let ws_others = room.players.clone().into_iter().map(|x| x.ws);
                                
                                // Add the player to the room
                                room.join(Player {
                                    username,
                                    avatar,
                                    ws: out.clone(),
                                });

                                println!("Room = {:?}", room);

                                // Send an update to each other player
                                for other in ws_others {
                                    other.send(&RoomUpdate {
                                        players: room.players.clone(),
                                    })?;
                                }

                                out.send(&OnRoomJoin {
                                    players: room.players.clone(),
                                })?;
                            }
                            StartRound { code } => {
                                // Get the room of given code

                                let mut rooms = ROOMS.lock().unwrap();
                                let mut room = match rooms.get_mut(&code) {
                                    Some(room) => room,
                                    None => return out.send(&Error {
                                        code: ErrorMsg::RoomNotFound,
                                    }),
                                };
                                room.votes.clear();
                                if room.questions_count > 9 {
                                    for player in &room.players {
                                        player.ws.send(&GameOver)?;
                                    }
                                } else {
                                    let question =
                                        &QUESTIONS[room.questions[room.questions_count as usize]];
                                    room.questions_count += 1;

                                    use rand::seq::SliceRandom;
                                    let mut rng = rand::thread_rng();
                                    let mut players_rand = room.players.clone();
                                    players_rand.shuffle(&mut rng);
                                    for i in 0..room.players.len() {
                                        room.players[i].ws.send(&NewRound {
                                            question: question
                                                .into_client(&players_rand[i].username),
                                        })?;
                                    }
                                }
                            }
                            Answer { code, vote } => {
                                let mut rooms = ROOMS.lock().unwrap();
                                let room = match rooms.get_mut(&code) {
                                    Some(room) => room,
                                    None => return out.send(&Error {
                                        code: ErrorMsg::RoomNotFound,
                                    }),
                                };

                                room.record_vote(vote);
                                let res = if room.votes.len() < room.players.len() {
                                    RoundUpdate {
                                        ready_player_count: room.votes.len() as u32,
                                    }
                                } else {
                                    RoundOver {
                                        votes: room.votes.clone(),
                                    }
                                };

                                for player in &room.players {
                                    player.ws.send(&res)?;
                                }
                            }
                        }

                        Ok(())
                    }
                })
                .unwrap();
            });
        }))
}

#[get("/")]
async fn index() -> Option<NamedFile> {
    NamedFile::open("static/index.html").await.ok()
}

impl GameRoom {
    fn create<R: rand::Rng + ?Sized>(code: String, questions: &[Prompt], rng: &mut R) -> Self {
        // Randomly choose 10 index for questions
        let v = (0..questions.len()).choose_multiple(rng, 10);
        Self {
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
