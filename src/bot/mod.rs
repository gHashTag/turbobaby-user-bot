pub mod commands;
pub mod callbacks;
pub mod handlers;

use teloxide::prelude::*;
use teloxide::dispatching::UpdateHandler;


pub fn create_handler() -> UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<commands::Command>()
                        .endpoint(commands::handle_command)
                )
                .branch(
                    Message::filter_text()
                        .endpoint(handlers::handle_text)
                )
                .branch(
                    dptree::filter(|msg: Message| msg.web_app_data().is_some())
                        .endpoint(handlers::handle_web_app_data)
                )
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(callbacks::handle_callback)
        )
}
