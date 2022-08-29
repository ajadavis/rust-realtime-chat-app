// this will enable us to use the rocket macros globally
#[macro_use]
extern crate rocket;

use rocket::{
    form::Form,
    fs::relative,
    fs::FileServer,
    response::stream::{Event, EventStream},
    serde::{Deserialize, Serialize},
    tokio::select,
    tokio::sync::broadcast::{channel, error::RecvError, Sender},
    Shutdown, State,
};

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]

struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

// Post Messages Endpoint
#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    // inside the fn we simply send the message to all receivers
    // the send method returns a result type b/c sending a message could fail
    // if there are no receivers. in this ex, we dont care about that case and will ignore
    let _res = queue.send(form.into_inner());
}

// Receive Messages Endpoint
#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();

    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };
            yield Event::json(&msg);
        }
    }
}

// the rocket fn will create a main fn that will start our rocket web server
#[launch]
fn rocket() -> _ {
    // build creates a new rocket server instance
    rocket::build()
        .manage(channel::<Message>(1024).0)
        // mount our routes
        .mount("/", routes![post, events])
        // mount a handler that will serve static files
        .mount("/", FileServer::from(relative!("static")))
}
