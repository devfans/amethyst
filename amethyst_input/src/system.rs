//! Input system

use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use amethyst_core::timing::Time;
use amethyst_core::shrev::{EventChannel, ReaderId};
use amethyst_core::specs::prelude::{Read, Resources, System, Write};
use std::hash::Hash;
use winit::Event;
use {Bindings, InputEvent, InputHandler};

/// Input system
///
/// Will read `winit::Event` from `EventHandler<winit::Event>`, process them with `InputHandler`,
/// and push the results in `EventHandler<InputEvent>`.
pub struct InputSystem<AX, AC>
where
    AX: Hash + Eq,
    AC: Hash + Eq,
{
    reader: Option<ReaderId<Event>>,
    bindings: Option<Bindings<AX, AC>>,
}

impl<AX, AC> InputSystem<AX, AC>
where
    AX: Hash + Eq,
    AC: Hash + Eq,
{
    /// Create a new input system. Needs a reader id for `EventHandler<winit::Event>`.
    pub fn new(bindings: Option<Bindings<AX, AC>>) -> Self {
        InputSystem {
            reader: None,
            bindings,
        }
    }

    fn process_event(
        event: &Event,
        handler: &mut InputHandler<AX, AC>,
        output: &mut EventChannel<InputEvent<AC>>,
    ) where
        AX: Hash + Eq + Clone + Send + Sync + 'static,
        AC: Hash + Eq + Clone + Send + Sync + 'static,
    {
        handler.send_event(event, output);
    }

    fn process_net_event(
        event: &InputEvent<AC>,
        handler: &mut InputHandler<AX, AC>,
        output: &mut EventChannel<InputEvent<AC>>,
    ) where
        AX: Hash + Eq + Clone + Send + Sync + 'static,
        AC: Hash + Eq + Clone + Send + Sync + 'static,
    {
        handler.send_net_event(event, output);
    }


}

impl<'a, AX, AC> System<'a> for InputSystem<AX, AC>
where
    AX: Hash + Eq + Clone + Send + Sync + 'static,
    AC: Hash + Eq + Clone + Send + Sync + 'static,
{
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        Write<'a, InputHandler<AX, AC>>,
        Write<'a, EventChannel<InputEvent<AC>>>,
        Write<'a, Time>,
        Read<'a, Option<Arc<Mutex<Receiver<Vec<InputEvent<AC>>>>>>>,
    );

    fn run(&mut self, (input, mut handler, mut output, mut time, rx): Self::SystemData) {
        /// Set Fixed time delta
        time.set_delta_time(Duration::from_millis(100));
        println!("Input system Tick with time delta: {}", time.delta_seconds());
        let net_rx = rx.as_ref().unwrap();
        let rx = net_rx.lock().unwrap();
        let events = rx.recv().unwrap();
        println!("Total {} net events", events.len());
        for event in events {
            Self::process_net_event(&event, &mut *handler, &mut *output);
        }
        // println!("Net_RX: {:?}", events);
        for event in input.read(&mut self.reader.as_mut().unwrap()) {
            println!("New event: {:?}", event);
            Self::process_event(event, &mut *handler, &mut *output);
        }
    }

    fn setup(&mut self, res: &mut Resources) {
        use amethyst_core::specs::prelude::SystemData;
        Self::SystemData::setup(res);
        self.reader = Some(res.fetch_mut::<EventChannel<Event>>().register_reader());
        if let Some(ref bindings) = self.bindings {
            res.fetch_mut::<InputHandler<AX, AC>>().bindings = bindings.clone();
        }
    }
}
