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
    HomeKit,
    Mqtt,
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
const TRANSITION_STEPS: u8 = 10;

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
    transition: Option<Transition>,
    seq: u64,
    tick: u64,
    cmd_rx: mpsc::Receiver<(Command, Source)>,
    snap_tx: watch::Sender<StateSnapshot>,
    frame_tx: broadcast::Sender<FrameMsg>,
}

#[derive(Clone, Debug)]
struct Transition {
    start_rgb: Rgb,
    target_rgb: Rgb,
    start_brightness: u8,
    target_brightness: u8,
    start_output_brightness: u8,
    target_output_brightness: u8,
    target_power: bool,
    step: u8,
    source: Source,
}

impl Transition {
    fn new(
        state: &State,
        target_rgb: Rgb,
        target_brightness: u8,
        target_power: bool,
        source: Source,
    ) -> Self {
        Self {
            start_rgb: state.rgb,
            target_rgb,
            start_brightness: state.brightness,
            target_brightness,
            start_output_brightness: state.effective_brightness(),
            target_output_brightness: if target_power {
                target_brightness.min(crate::state::MAX_BRIGHTNESS)
            } else {
                0
            },
            target_power,
            step: 0,
            source,
        }
    }

    fn targets(&self) -> (Rgb, u8, bool) {
        (self.target_rgb, self.target_brightness, self.target_power)
    }

    fn output_brightness(&self) -> u8 {
        if self.step >= TRANSITION_STEPS {
            return self.target_output_brightness;
        }
        lerp_u8(
            self.start_output_brightness,
            self.target_output_brightness,
            self.step,
        )
    }

    fn advance(&mut self, state: &mut State) -> bool {
        self.step = self.step.saturating_add(1);
        let done = self.step >= TRANSITION_STEPS;
        if done {
            state.rgb = self.target_rgb;
            state.brightness = self.target_brightness;
            state.power = self.target_power;
            return true;
        }

        if self.target_power {
            state.power = true;
        }
        state.rgb = Rgb {
            r: lerp_u8(self.start_rgb.r, self.target_rgb.r, self.step),
            g: lerp_u8(self.start_rgb.g, self.target_rgb.g, self.step),
            b: lerp_u8(self.start_rgb.b, self.target_rgb.b, self.step),
        };
        state.brightness = lerp_u8(self.start_brightness, self.target_brightness, self.step);
        false
    }
}

fn lerp_u8(from: u8, to: u8, step: u8) -> u8 {
    let from = i16::from(from);
    let to = i16::from(to);
    let value = from
        + ((to - from) * i16::from(step) + i16::from(TRANSITION_STEPS / 2))
            / i16::from(TRANSITION_STEPS);
    value.clamp(0, 255) as u8
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
        // seq == 0 is the reserved boot snapshot (never produced by a command).
        let snapshot = StateSnapshot {
            state: state.clone(),
            seq: 0,
            source: Source::Internal,
        };
        let (snap_tx, snap_rx) = watch::channel(snapshot);
        let (frame_tx, _) = broadcast::channel(FRAME_CAPACITY);
        let handle = EngineHandle {
            commands: cmd_tx,
            snapshots: snap_rx,
            frames: frame_tx.clone(),
        };
        let engine = Engine {
            display,
            state,
            transition: None,
            seq: 0,
            tick: 0,
            cmd_rx,
            snap_tx,
            frame_tx,
        };
        (handle, engine)
    }

    #[cfg(test)]
    pub fn display_ref(&self) -> &D {
        &self.display
    }

    /// Render the current state once (used by tests and the solid path).
    pub fn render_once(&mut self) {
        let frame = render_frame(&self.state, self.tick);
        let _ = self.display.show(&frame, self.render_brightness());
        let _ = self.frame_tx.send(frame);
    }

    /// Apply one command, bump seq, and broadcast the new snapshot.
    /// Single place for seq/snapshot logic so it can't drift.
    fn handle_command(&mut self, cmd: Command, source: Source) {
        match cmd {
            Command::SetPower(power) => {
                let (target_rgb, target_brightness, _) = self.transition_targets();
                self.start_transition(target_rgb, target_brightness, power, source);
            }
            Command::SetColor(rgb) => {
                let (_, target_brightness, target_power) = self.transition_targets();
                self.start_transition(rgb, target_brightness, target_power, source);
            }
            Command::SetBrightness(brightness) => {
                let (target_rgb, _, target_power) = self.transition_targets();
                self.start_transition(target_rgb, brightness, target_power, source);
            }
            other => {
                self.transition = None;
                apply_command(&mut self.state, other);
                self.broadcast_snapshot(source);
            }
        }
    }

    fn render_brightness(&self) -> u8 {
        self.transition
            .as_ref()
            .map(Transition::output_brightness)
            .unwrap_or_else(|| self.state.effective_brightness())
    }

    fn transition_targets(&self) -> (Rgb, u8, bool) {
        self.transition
            .as_ref()
            .map(Transition::targets)
            .unwrap_or((self.state.rgb, self.state.brightness, self.state.power))
    }

    fn start_transition(
        &mut self,
        target_rgb: Rgb,
        target_brightness: u8,
        target_power: bool,
        source: Source,
    ) {
        self.state.mode = Mode::Solid;
        if self.state.rgb == target_rgb
            && self.state.brightness == target_brightness
            && self.state.power == target_power
        {
            self.transition = None;
        } else {
            self.transition = Some(Transition::new(
                &self.state,
                target_rgb,
                target_brightness,
                target_power,
                source,
            ));
        }
        if target_power {
            self.state.power = true;
        }
        self.broadcast_snapshot(source);
    }

    fn advance_transition(&mut self) {
        if let Some(transition) = &mut self.transition {
            let source = transition.source;
            let done = transition.advance(&mut self.state);
            if done {
                self.transition = None;
            }
            self.broadcast_snapshot(source);
        }
    }

    fn broadcast_snapshot(&mut self, source: Source) {
        self.seq += 1;
        let _ = self.snap_tx.send(StateSnapshot {
            state: self.state.clone(),
            seq: self.seq,
            source,
        });
    }

    fn drain_commands(&mut self) -> bool {
        let mut changed = false;
        while let Ok((cmd, source)) = self.cmd_rx.try_recv() {
            self.handle_command(cmd, source);
            changed = true;
        }
        changed
    }

    /// The single-writer run loop. Owns the Display for its whole life.
    pub async fn run(mut self) {
        use tokio::time::{interval, Duration, MissedTickBehavior};
        let mut ticker = interval(Duration::from_millis(33)); // ~30 fps
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // Render (and broadcast) the restored/default state at startup so a
        // Solid-mode boot shows immediately, before any command arrives.
        self.render_once();
        loop {
            let animating = (matches!(self.state.mode, Mode::Effect) && self.state.power)
                || self.transition.is_some();
            if animating {
                tokio::select! {
                    _ = ticker.tick() => {
                        self.tick = self.tick.wrapping_add(1);
                        self.advance_transition();
                    }
                    n = self.cmd_rx.recv() => {
                        match n {
                            Some((cmd, source)) => self.handle_command(cmd, source),
                            None => break, // all senders dropped -> shutdown
                        }
                        self.drain_commands();
                    }
                }
            } else {
                match self.cmd_rx.recv().await {
                    Some((cmd, source)) => {
                        self.handle_command(cmd, source);
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

impl Engine<Box<dyn crate::display::Display>> {
    pub fn new_boxed(
        display: Box<dyn crate::display::Display>,
        state: State,
    ) -> (EngineHandle, Engine<Box<dyn crate::display::Display>>) {
        Engine::new(display, state)
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
        let mut s = crate::state::State {
            mode: Mode::Effect,
            ..Default::default()
        };
        apply(&mut s, Command::SetColor(Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(s.mode, Mode::Solid);
        assert_eq!(s.rgb, Rgb { r: 1, g: 2, b: 3 });
    }

    #[test]
    fn set_effect_switches_mode_and_keeps_last_speed_when_none() {
        let mut s = crate::state::State {
            speed: 77,
            ..Default::default()
        };
        apply(
            &mut s,
            Command::SetEffect {
                name: "rainbow".into(),
                speed: None,
            },
        );
        assert_eq!(s.mode, Mode::Effect);
        assert_eq!(s.effect_name, "rainbow");
        assert_eq!(s.speed, 77, "omitted speed keeps previous value");
    }

    #[test]
    fn power_gate_blanks_output_but_keeps_state() {
        let mut s = crate::state::State {
            power: true,
            brightness: 100,
            ..Default::default()
        };
        apply(&mut s, Command::SetPower(false));
        assert!(!s.power);
        assert_eq!(s.brightness, 100);
        assert_eq!(s.effective_brightness(), 0);
    }

    #[test]
    fn color_and_brightness_commands_transition_to_one_target() {
        let initial = crate::state::State {
            rgb: Rgb { r: 0, g: 0, b: 255 },
            brightness: 100,
            ..Default::default()
        };
        let (_, mut engine) = Engine::new(MockDisplay::new(), initial);

        engine.handle_command(
            Command::SetColor(Rgb {
                r: 255,
                g: 32,
                b: 0,
            }),
            Source::Rest,
        );
        engine.handle_command(Command::SetBrightness(30), Source::Rest);

        assert_ne!(
            engine.state.rgb,
            Rgb {
                r: 255,
                g: 32,
                b: 0
            }
        );
        assert_ne!(engine.state.brightness, 30);
        assert!(engine.transition.is_some());

        for _ in 0..TRANSITION_STEPS {
            engine.advance_transition();
        }

        assert_eq!(
            engine.state.rgb,
            Rgb {
                r: 255,
                g: 32,
                b: 0
            }
        );
        assert_eq!(engine.state.brightness, 30);
        assert!(engine.transition.is_none());
    }

    #[test]
    fn brightness_transition_does_not_jump_to_target() {
        let initial = crate::state::State {
            brightness: 100,
            ..Default::default()
        };
        let (_, mut engine) = Engine::new(MockDisplay::new(), initial);

        engine.handle_command(Command::SetBrightness(30), Source::Rest);

        assert_eq!(engine.state.brightness, 100);
        engine.advance_transition();
        assert!(engine.state.brightness < 100);
        assert!(engine.state.brightness > 30);
    }

    #[test]
    fn power_off_fades_output_without_losing_brightness() {
        let initial = crate::state::State {
            power: true,
            rgb: Rgb {
                r: 80,
                g: 120,
                b: 200,
            },
            brightness: 100,
            ..Default::default()
        };
        let (_, mut engine) = Engine::new(MockDisplay::new(), initial);

        engine.handle_command(Command::SetPower(false), Source::Rest);

        assert!(
            engine.state.power,
            "lights stay logically on during fade-out"
        );
        assert_eq!(engine.state.brightness, 100);
        assert_eq!(engine.render_brightness(), 100);

        engine.advance_transition();
        assert!(engine.state.power);
        assert_eq!(engine.state.brightness, 100);
        assert!(engine.render_brightness() < 100);
        assert!(engine.render_brightness() > 0);

        for _ in 1..TRANSITION_STEPS {
            engine.advance_transition();
        }

        assert!(!engine.state.power);
        assert_eq!(engine.state.brightness, 100);
        assert_eq!(engine.render_brightness(), 0);
        assert!(engine.transition.is_none());
    }

    #[test]
    fn power_on_fades_from_black_to_saved_brightness() {
        let initial = crate::state::State {
            power: false,
            rgb: Rgb {
                r: 80,
                g: 120,
                b: 200,
            },
            brightness: 90,
            ..Default::default()
        };
        let (_, mut engine) = Engine::new(MockDisplay::new(), initial);

        engine.handle_command(Command::SetPower(true), Source::Rest);

        assert!(engine.state.power, "transition must render while fading in");
        assert_eq!(engine.state.brightness, 90);
        assert_eq!(engine.render_brightness(), 0);

        engine.advance_transition();
        assert!(engine.state.power);
        assert_eq!(engine.state.brightness, 90);
        assert!(engine.render_brightness() > 0);
        assert!(engine.render_brightness() < 90);

        for _ in 1..TRANSITION_STEPS {
            engine.advance_transition();
        }

        assert!(engine.state.power);
        assert_eq!(engine.state.brightness, 90);
        assert_eq!(engine.render_brightness(), 90);
        assert!(engine.transition.is_none());
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

    #[tokio::test]
    async fn engine_broadcasts_initial_frame_on_startup() {
        let (handle, engine) = Engine::new(MockDisplay::new(), crate::state::State::default());
        // Subscribe BEFORE spawning so we don't miss the boot frame.
        let mut frames = handle.subscribe_frames();
        tokio::spawn(engine.run());
        // No command sent: the initial render must still arrive.
        let got = tokio::time::timeout(std::time::Duration::from_millis(500), frames.recv()).await;
        let frame = got
            .expect("timed out waiting for boot frame")
            .expect("frame channel closed");
        assert_eq!(frame.len(), crate::geometry::PIXEL_COUNT);
        drop(handle);
    }
}
