#[macro_use] extern crate rocket;
use rocket::response::NamedFile;
use std::sync::Mutex;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize)]
struct Player {
    username: String,
    avatar: String,
    #[serde(skip)]
    ws: ws::Sender,
}
type Question = String;
type Vote = (String, String);

#[derive(Debug)]
struct Room {
    code: String,
    players: Vec<Player>
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
}

#[derive(Deserialize)]
#[serde(tag = "tag")]
enum ClientEvent {
    /***** Client *****/
    /// Client ask Server to create a room
    CreateRoom, //TODO règles
    /// When Client joins a room
    JoinRoom {
        username: String,
        avatar: String,
        code: String,
    },
    /// When the admin of the room starts the game
    StartGame {
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
    static ref ROOMS: Mutex<HashMap<String,Room>> = Mutex::new(HashMap::new());
}

fn send_msg(out: &ws::Sender, msg: &ServerEvent) -> ws::Result<()> {
    let msg = serde_json::to_string(&msg).unwrap();
    out.send(ws::Message::Text(msg))
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
                ws::listen("0.0.0.0:8008", |out| {
                    move |msg| {
                        use ServerEvent::*;
                        use ClientEvent::*;

                        let msg = match msg {
                            ws::Message::Text(s) => s,
                            _ => unreachable!(),
                        };
                        let evt : ClientEvent = serde_json::from_str(&msg).unwrap();
                        let response = match evt {
                            CreateRoom => {
                                let room = Room::create();
                                let res = RoomCreated { code: room.code.clone() };

                                ROOMS.lock()
                                    .unwrap()
                                    .insert(room.code.clone(), room);

                                res
                            },
                            JoinRoom {
                                username, avatar, code
                            } => {
                                // Get the room of given code
                                let mut rooms = ROOMS.lock().unwrap();
                                let room = rooms.get_mut(&code).unwrap();
                                    
                                // Fetch the websocket Sender element for each player, in order to send a RoomUpdate event
                                let ws_others = room.players.clone().into_iter().map(|x| x.ws);

                                // Add the player to the room
                                room.players.push(Player { username, avatar, ws: out.clone() });
                                
                                println!("Room = {:?}", room);

                                // Send an update to each other player
                                for other in ws_others {
                                    send_msg(&other, &RoomUpdate { players: room.players.clone() })?;
                                }

                                OnRoomJoin { players: room.players.clone() }
                            },
                            StartGame { code: _ } => todo!(),
                            Answer { vote: _ } => todo!(),
                            NewGame => todo!(),
                        };
                        
                        send_msg(&out, &response)
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

    fn join(&mut self, player : Player) {
        self.players.push(player);
    }
}