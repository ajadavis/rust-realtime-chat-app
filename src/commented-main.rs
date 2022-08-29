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

// our Message struct derives a few traits:
// debug so this struct can be printed out with debug format
// clone so we can duplicate messages
// fromform so we can take form data and transform it into a message struct
// Serialize/Deserialize whihc will allow this data atruct to be Serialize/Deserialize
#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
// serialization will happen via serde
// state we want to use serde crate defined in rocket
#[serde(crate = "rocket::serde")]

// implement the Message struct - 3 fields, string w/ extra validation for room and usrname
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

//
//
//
// we need two endpoints, one to post msgs and one to receive msgs

// Post Messages Endpoint
// route is /message and accepts form data
#[post("/message", data = "<form>")]
// the function handler accepts two arguments:
// the form data which will be converted to the message struct
// and the server state which will be a Sender that can send Messages
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    // inside the fn we simply send the message to all receivers
    // the send method returns a result type b/c sending a message could fail
    // if there are no receivers. in this ex, we dont care about that case and will ignore
    let _res = queue.send(form.into_inner());
}

//
//
//
//
// next we will set up the get route to receive messages
// we handle get requets to the /events path
#[get("/events")]
// the return type is an infinite stream of server send events
// server send events allow a client to open a long live connection with a server
// and then the server can send data to the clients whenever it wants - like a 
// one-way websocket.
// this fn is async b/c server send events are produced asyncronously 
// the handler takes two args:
// 1. queue which is our server state 
// 2. and end which is type Shutdown - a future that resolves when 
// our server instance is shutdown
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {

    // inside handler first thing we do is create a new receiver with queue.subscribe
    // this allows us to listen for messages when they are sent down the channel
    let mut rx = queue.subscribe();

    // next we use generator syntax to produce and infinite stream of server send events
    EventStream! {
        // infinte loop 
        loop {
            // first we use select macro:
            // select waits on multi concurrent branches and returns
            // when one of them completes. We only have 2 branches here.
            let msg = select! {
                // branch 1 calling receive on our receiver which waits for new msgs
                // when we get a new message, we map to variable "msg", then match against that
                msg = rx.recv() => match msg {
                    // receiver returns a result enum
                    // if we get OK variant, we return the msg
                    Ok(msg) => msg,
                    // if we get error variant and error=closed, there are no more senders
                    // so we can break out of the infinite loop
                    Err(RecvError::Closed) => break,
                    // if we get error variant and error=lag, it means our receiver lagged
                    // too far behind and was forceably disconnected, in that case we simply
                    // skip to the new iteration of loop

                    Err(RecvError::Lagged(_)) => continue,
                },
                // this is the second branch, waiting for the shutdown future to resolve
                // the shutdown future resolves when our server is notified to shutdown
                // at whichi point we can break out of this infinite loop.
                _ = &mut end => break,
            };

            // assuming so errors, the select will return a msg.
            // we can yeild a new server sent event
            // passing in our message
            yield Event::json(&msg); 
        } 
    }
}

// the rocket fn will create a main fn that will start our rocket web server
#[launch]
fn rocket() -> _ {
    // build creates a new rocket server instance
    rocket::build()
        // manage method allows us to add stater to our rocket instance
        // which all handlers have access to.
        // The state we add is a channel state - specifically the sender end
        // rocket uses tokio as an async runtime, and channels are a way
        // to pass messages between differnt async tasks.
        // We specify what type of messages we want to pass in our channel:
        // in this case specifically a Message struct defined above
        // we also pass in a capacity: the amt of msgs a channel can retain at a given time
        // note: the return value of calling the channel function is a tuple
        // containing a send and receiver end.
        // we write .0 at the end to get the first element of tuple
        // b/c we only want to store the sender end in state
        .manage(channel::<Message>(1024).0)
        // mount our routes - takes two args:
        // 1. base acts the namespace for routes and
        // 2. list of routes - use routes! marco and pass names of handler fn's
        .mount("/", routes![post, events])
        // mount a handler that will serve static files
        .mount("/", FileServer::from(relative!("static")))
}
 