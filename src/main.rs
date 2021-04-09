#[macro_use] extern crate rocket;
use rocket::response::NamedFile;
use rocket::State;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

type Player = String;
type Question = String;
type Vote = (Player, Player);

struct Room {
    code: String,
    players: Vec<Player>
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "tag")]
enum WsEvent {
    /***** Server *****/
    /// When Server created a room and tells Client that the room is ready
    RoomCreated {
        code: String,
    },
    /// When Server sends all the infos about the room to a Client that has just joined
    OnRoomJoin {
        code: String,
        players: Vec<Player>,
        // TODO règles
    },
    /// When Server tells Clients that something has changed in the room (e.g. new player)
    RoomUpdate {
        players: Vec<Player>,
    },
    /// When Server tells Clients that the game has started
    GameStart,
    /// When Server tells the new round's infos to Clients
    NewRound {
        question: Question,
    },
    /// When Server tells Clients that something has changed in the round (e.g. X players have responded)
    RoundUpdate {
        ready_player_count: u32,
    },
    /// When Server tells Clients that the round is over (the time is over / everyone responded)
    RoundOver {
        votes: Vec<Vote>,
    },
    /***** Client *****/
    /// Client ask Server to create a room
    CreateRoom, //TODO règles
    /// When Client joins a room
    JoinRoom {
        username: String,
        avatar: String,
        code: String,
    },
    /// When Client answers to a question
    Answer {
        vote: Vote,
    },
    /// Request to the server to start the game with the same room
    NewGame,    

}

lazy_static::lazy_static! {
    static ref ROOMS: Mutex<Vec<Room>> = Mutex::new(vec![]);
}

#[rocket::launch]
fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .mount("/", routes![
            index,
        ])
        .mount("/static", rocket_contrib::serve::StaticFiles::from("static"))
        .attach(rocket::fairing::AdHoc::on_launch("WebSocket", |_| {
            std::thread::spawn(|| {
                ws::listen("127.0.0.1:8008", |out| {
                    move |msg| {
                        //out.send(msg)
                        use WsEvent::*;
                        let msg = match msg {
                            ws::Message::Text(s) => s,
                            _ => unreachable!(),
                        };
                        let evt : WsEvent = serde_json::from_str(&msg).unwrap();
                        match evt {
                            CreateRoom => {
                                println!("creating room");
                                let room = Room::create();
                                let msg = serde_json::to_string(&RoomCreated{ code: room.code.clone() }).unwrap();

                                ROOMS.lock()
                                    .unwrap()
                                    .push(room);

                                out.send(ws::Message::Text(msg))?;
                            },
                            JoinRoom {
                                username, avatar: _, code
                            } => {
                                println!("room {} joined by {}", code, username);
                            },
                            Answer { vote: _ } => todo!(),
                            NewGame => todo!(),
                            _ => unreachable!(),
                        }

                        Ok(())
                    }
                }).unwrap();
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
            players: vec![]
        }
    }
}