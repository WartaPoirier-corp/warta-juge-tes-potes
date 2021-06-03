#[macro_use]
extern crate rocket;

pub mod id_gen;

use rand::seq::IteratorRandom;
use rocket::response::NamedFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Mutex;

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

// For a question : First username = Player who voted, second username = Choice
// For a Tag : First username = Player who voted, second username = Choice
type Vote = (Username, Username);
type Username = String;

#[derive(Debug)]
struct Room {
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
    static ref ROOMS: Mutex<HashMap<String,Room>> = Mutex::new(HashMap::new());
}

fn send_msg(out: &ws::Sender, msg: &ServerEvent) -> ws::Result<()> {
    let msg = serde_json::to_string(&msg).unwrap();
    out.send(ws::Message::Text(msg))
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

                        // TODO: c pa opti
                        let content = std::fs::read_to_string("questions.ron").unwrap();
                        let questions: Vec<Prompt> = ron::from_str(&content).unwrap();

                        let msg = match msg {
                            ws::Message::Text(s) => s,
                            _ => unreachable!(),
                        };
                        let evt: ClientEvent = serde_json::from_str(&msg).unwrap();
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
                                let room = Room::create(code, &questions, &mut rng);

                                let res = RoomCreated {
                                    code: room.code.clone(),
                                };

                                rooms.insert(room.code.clone(), room);

                                send_msg(&out, &res)?;
                            }
                            JoinRoom {
                                username,
                                avatar,
                                code,
                            } => {
                                // Get the room of given code
                                let mut rooms = ROOMS.lock().unwrap();
                                let room = rooms.get_mut(&code).unwrap();

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
                                    send_msg(
                                        &other,
                                        &RoomUpdate {
                                            players: room.players.clone(),
                                        },
                                    )?;
                                }

                                send_msg(
                                    &out,
                                    &OnRoomJoin {
                                        players: room.players.clone(),
                                    },
                                )?;
                            }
                            StartRound { code } => {
                                // Get the room of given code

                                let mut rooms = ROOMS.lock().unwrap();
                                let mut room = rooms.get_mut(&code).unwrap();
                                room.votes.clear();
                                if room.questions_count > 9 {
                                    for player in &room.players {
                                        send_msg(&player.ws, &GameOver)?;
                                    }
                                } else {
                                    let question =
                                        &questions[room.questions[room.questions_count as usize]];
                                    room.questions_count += 1;

                                    use rand::seq::SliceRandom;
                                    let mut rng = rand::thread_rng();
                                    let mut players_rand = room.players.clone();
                                    players_rand.shuffle(&mut rng);
                                    for i in 0..room.players.len() {
                                        send_msg(
                                            &room.players[i].ws,
                                            &NewRound {
                                                question: question
                                                    .into_client(&players_rand[i].username),
                                            },
                                        )?;
                                    }
                                }
                            }
                            Answer { code, vote } => {
                                let mut rooms = ROOMS.lock().unwrap();
                                let room = rooms.get_mut(&code).unwrap();
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
                                    send_msg(&player.ws, &res)?;
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
async fn index() -> NamedFile {
    NamedFile::open("static/index.html").await.unwrap()
}

impl Room {
    fn create<R: rand::Rng + ?Sized>(code: String, questions: &[Prompt], rng: &mut R) -> Room {
        // Randomly choose 10 index for questions
        let v = (0..questions.len()).choose_multiple(rng, 10);
        Room {
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
