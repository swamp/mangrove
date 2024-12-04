/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use limnus_message::prelude::Message;
use mangrove_script::ScriptMessage;
use message_channel::{Channel, Receiver};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use swamp::prelude::{App, LoReM, LocalResource, MsgM, Plugin, UpdatePhase};

#[derive(LocalResource, Debug)]
pub struct FileWatcher {
    pub receiver: Receiver<SaveDetectorMessage>,
    pub watcher: RecommendedWatcher, // keeps watcher alive
}

impl FileWatcher {}

pub fn start_watch(
    watch_path: PathBuf,
) -> notify::Result<(RecommendedWatcher, Receiver<SaveDetectorMessage>)> {
    let (sender, receiver) = Channel::create();

    let mut last_event = Instant::now() - Duration::from_secs(1);
    let debounce_duration = Duration::from_millis(100); // Might need to tweak this

    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(_event) => {
            let now = Instant::now();
            if now.duration_since(last_event) >= debounce_duration {
                sender
                    .send(SaveDetectorMessage::Changed)
                    .expect("TODO: panic message");
                last_event = now;
            }
        }
        Err(e) => println!("watch error: {:?}", e),
    })?;

    // Start watching the directory
    watcher.watch(&watch_path, RecursiveMode::Recursive)?;

    Ok((watcher, receiver))
}

pub fn tick(mut detector_message: MsgM<ScriptMessage>, file_watcher: LoReM<FileWatcher>) {
    while let Ok(_found) = file_watcher.receiver.recv() {
        detector_message.send(ScriptMessage::Reload);
    }
}

#[derive(Debug, Message)]
pub enum SaveDetectorMessage {
    Changed,
}

pub struct SaveDetectorPlugin;

impl Plugin for SaveDetectorPlugin {
    fn build(&self, app: &mut App) {
        app.create_message_type::<SaveDetectorMessage>();
        let (watcher, receiver) =
            start_watch(Path::new("scripts/").to_path_buf()).expect("TODO: panic message");

        let file_watcher = FileWatcher { receiver, watcher };
        app.insert_local_resource(file_watcher);
        app.add_system(UpdatePhase::Update, tick);
    }
}
