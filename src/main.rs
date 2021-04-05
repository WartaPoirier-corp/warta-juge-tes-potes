#[macro_use] extern crate rocket;
use rocket::response::NamedFile;
use std::env;
use tokio::net::{TcpListener, TcpStream};
use rocket::futures::{StreamExt, TryStreamExt};
use std::net::SocketAddr;

type Player = String;
type Question = String;

enum WsEvent {
    /***** Server *****/
    /// When Server created a room and tells Client that the room is ready
    RoomCreated {
        code: String,
    },
    /// When Server sends all the infos about the room to a Client that has just joined
    RoomJoin {
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
    RoundOver,
    


}

#[tokio::main]
async fn main() {
    tokio::join!(
        websocket(),
        rocket().launch(),
    ).1.ok();
}

async fn websocket() {
    // Fetch an adress for the server, if there is none take a default one
    let addr = match env::args().nth(1) {
        Some(address) => address,
        None => "127.0.0.1:8008".to_string()        
    };
    // Create a TcpListener and binds it to the adress defined above. Used for websockets
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind WS");
    println!("Listening on: {}", addr);

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr));
    }
}

async fn handle_connection(raw_stream: TcpStream, addr: SocketAddr) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        println!("Received a message from {}: {}", addr, msg.to_text().unwrap());

        // outgoing.unbounded_send(msg.clone()).unwrap();

        std::future::ready(Ok(()))
    }).await;

    println!("{} disconnected", &addr);
}

fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .mount("/", routes![
            index,
            js,
            css,
        ])
}

#[get("/")]
async fn index() -> NamedFile {
    NamedFile::open("static/index.html").await.unwrap()
}

#[get("/main.css")]
async fn css() -> NamedFile {
    NamedFile::open("static/main.css").await.unwrap()
}

#[get("/main.js")]
async fn js() -> NamedFile {
    NamedFile::open("static/main.js").await.unwrap()
}