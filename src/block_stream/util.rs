use std::fmt::Debug;

use futures::channel::mpsc::Sender;

/// Send a message down the channel or print to stderr if the channel is disconnected.
pub fn send_or_eprint<T: Debug>(mut message: T, tx: &mut Sender<T>) {
    while let Err(error) = tx.try_send(message) {
        if error.is_disconnected() {
            eprintln!(
                "Channel disconnected. Failed to send {:?}",
                error.into_inner(),
            );
            return;
        } else {
            message = error.into_inner();
        }
    }
}
