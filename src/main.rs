#[macro_use]
extern crate rocket;
use rocket::response::NamedFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use rand::seq::IteratorRandom;

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
// First username : Player who voted, second username: Choice
type Vote = (Username, Username);
type Username = String;

#[derive(Debug)]
struct Room {
    code: String,
    players: Vec<Player>,
    votes: Vec<Vote>,
    questions_count: u32,
}

#[derive(Serialize)]
#[serde(tag = "tag")]
enum ServerEvent {
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
        question: String,
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
            std::thread::spawn(|| {
                ws::listen("0.0.0.0:8008", |out| {
                    move |msg| {
                        use ClientEvent::*;
                        use ServerEvent::*;

                        // TODO: c pa opti
                        let content = std::fs::read_to_string("questions.ron").unwrap();
                        let questions : Vec<Prompt> = ron::from_str(&content).unwrap();

                        let msg = match msg {
                            ws::Message::Text(s) => s,
                            _ => unreachable!(),
                        };
                        let evt: ClientEvent = serde_json::from_str(&msg).unwrap();
                        match evt {
                            CreateRoom => {
                                let room = Room::create();
                                let res = RoomCreated {
                                    code: room.code.clone(),
                                };

                                ROOMS.lock().unwrap().insert(room.code.clone(), room);

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
                                    println!("Test");
                                    for player in &room.players {
                                        send_msg(&player.ws, &GameOver)?;
                                    }
                                } else {
                                    room.questions_count += 1;
                                    let mut rng = rand::thread_rng();
                                    let mut question = questions.iter().choose(&mut rng).unwrap();
                                    while let Prompt::Tag(_, _) = question {
                                        question = questions.iter().choose(&mut rng).unwrap();
                                    }
                                    if let Prompt::Question(question) = question {
                                        for player in &room.players {
                                            send_msg(
                                                &player.ws,
                                                &NewRound {
                                                    question: question.clone(),
                                                },
                                            )?;
                                        }
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
    fn create() -> Room {
        Room {
            code: "BOUYA123".to_string(),
            players: vec![],
            votes: vec![],
            questions_count: 0,
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
