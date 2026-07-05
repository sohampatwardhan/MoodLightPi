use crate::color::Rgb;
use crate::display::Display;
use crate::effects::render_frame;
use crate::state::{Mode, State};
use tokio::sync::{broadcast, mpsc, watch};

/// Who originated a command/snapshot (for future adapter echo-suppression).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Rest,
    WebSocket,
    Internal,
}

#[derive(Clone, Debug)]
pub enum Command {
    SetPower(bool),
    SetColor(Rgb),
    SetBrightness(u8),
    SetEffect { name: String, speed: Option<u8> },
}

#[derive(Clone, Debug)]
pub struct StateSnapshot {
    pub state: State,
    pub seq: u64,
    pub source: Source,
}

/// Frame pushed to WS clients (logical order).
pub type FrameMsg = crate::geometry::Frame;

pub const COMMAND_CAPACITY: usize = 32;
const FRAME_CAPACITY: usize = 8;

/// Cloneable handle adapters use to reach the engine.
#[derive(Clone)]
pub struct EngineHandle {
    pub commands: mpsc::Sender<(Command, Source)>,
    pub snapshots: watch::Receiver<StateSnapshot>,
    pub frames: broadcast::Sender<FrameMsg>,
}

impl EngineHandle {
    pub fn current(&self) -> State {
        self.snapshots.borrow().state.clone()
    }
    pub fn subscribe_frames(&self) -> broadcast::Receiver<FrameMsg> {
        self.frames.subscribe()
    }
}

pub struct Engine<D: Display> {
    display: D,
    state: State,
    seq: u64,
    tick: u64,
    cmd_rx: mpsc::Receiver<(Command, Source)>,
    snap_tx: watch::Sender<StateSnapshot>,
    frame_tx: broadcast::Sender<FrameMsg>,
}

/// Pure state transition — unit tested directly.
pub fn apply_command(state: &mut State, cmd: Command) {
    match cmd {
        Command::SetPower(on) => state.power = on,
        Command::SetColor(rgb) => {
            state.rgb = rgb;
            state.mode = Mode::Solid;
        }
        Command::SetBrightness(v) => state.brightness = v,
        Command::SetEffect { name, speed } => {
            state.effect_name = name;
            state.mode = Mode::Effect;
            if let Some(s) = speed {
                state.speed = s;
            }
        }
    }
}

impl<D: Display> Engine<D> {
    pub fn new(display: D, state: State) -> (EngineHandle, Engine<D>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CAPACITY);
        let snapshot = StateSnapshot { state: state.clone(), seq: 0, source: Source::Internal };
        let (snap_tx, snap_rx) = watch::channel(snapshot);
        let (frame_tx, _) = broadcast::channel(FRAME_CAPACITY);
        let handle = EngineHandle {
            commands: cmd_tx,
            snapshots: snap_rx,
            frames: frame_tx.clone(),
        };
        let engine = Engine {
            display, state, seq: 0, tick: 0, cmd_rx, snap_tx, frame_tx,
        };
        (handle, engine)
    }

    #[cfg(test)]
    pub fn display_ref(&self) -> &D { &self.display }

    /// Render the current state once (used by tests and the solid path).
    pub fn render_once(&mut self) {
        let frame = render_frame(&self.state, self.tick);
        let _ = self.display.show(&frame, self.state.effective_brightness());
        let _ = self.frame_tx.send(frame);
    }

    fn drain_commands(&mut self) -> bool {
        let mut changed = false;
        while let Ok((cmd, source)) = self.cmd_rx.try_recv() {
            apply_command(&mut self.state, cmd);
            self.seq += 1;
            let _ = self.snap_tx.send(StateSnapshot {
                state: self.state.clone(), seq: self.seq, source,
            });
            changed = true;
        }
        changed
    }

    /// The single-writer run loop. Owns the Display for its whole life.
    pub async fn run(mut self) {
        use tokio::time::{interval, Duration, MissedTickBehavior};
        let mut ticker = interval(Duration::from_millis(33)); // ~30 fps
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let animating = matches!(self.state.mode, Mode::Effect) && self.state.power;
            if animating {
                tokio::select! {
                    _ = ticker.tick() => { self.tick = self.tick.wrapping_add(1); }
                    n = self.cmd_rx.recv() => {
                        match n {
                            Some((cmd, source)) => {
                                apply_command(&mut self.state, cmd);
                                self.seq += 1;
                                let _ = self.snap_tx.send(StateSnapshot {
                                    state: self.state.clone(), seq: self.seq, source });
                            }
                            None => break, // all senders dropped -> shutdown
                        }
                        self.drain_commands();
                    }
                }
            } else {
                match self.cmd_rx.recv().await {
                    Some((cmd, source)) => {
                        apply_command(&mut self.state, cmd);
                        self.seq += 1;
                        let _ = self.snap_tx.send(StateSnapshot {
                            state: self.state.clone(), seq: self.seq, source });
                        self.drain_commands();
                    }
                    None => break,
                }
            }
            self.render_once();
        }
        let _ = self.display.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::display::MockDisplay;
    use crate::state::Mode;

    fn apply(state: &mut crate::state::State, cmd: Command) {
        super::apply_command(state, cmd);
    }

    #[test]
    fn set_color_forces_solid_mode() {
        let mut s = crate::state::State { mode: Mode::Effect, ..Default::default() };
        apply(&mut s, Command::SetColor(Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(s.mode, Mode::Solid);
        assert_eq!(s.rgb, Rgb { r: 1, g: 2, b: 3 });
    }

    #[test]
    fn set_effect_switches_mode_and_keeps_last_speed_when_none() {
        let mut s = crate::state::State { speed: 77, ..Default::default() };
        apply(&mut s, Command::SetEffect { name: "rainbow".into(), speed: None });
        assert_eq!(s.mode, Mode::Effect);
        assert_eq!(s.effect_name, "rainbow");
        assert_eq!(s.speed, 77, "omitted speed keeps previous value");
    }

    #[test]
    fn power_gate_blanks_output_but_keeps_state() {
        let mut s = crate::state::State { power: true, brightness: 100, ..Default::default() };
        apply(&mut s, Command::SetPower(false));
        assert!(!s.power);
        assert_eq!(s.brightness, 100);
        assert_eq!(s.effective_brightness(), 0);
    }

    #[tokio::test]
    async fn engine_renders_a_frame_to_the_display() {
        let (handle, mut engine) = Engine::new(MockDisplay::new(), crate::state::State::default());
        engine.render_once();
        let strip = engine.display_ref().last_strip();
        let idx = crate::geometry::xy_to_index(0, 0);
        assert_ne!(strip[idx], [0, 0, 0]);
        drop(handle);
    }
}
