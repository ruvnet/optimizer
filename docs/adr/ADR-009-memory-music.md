# ADR-009: Memory Music - Generative Audio from RAM Patterns

## Status
Proposed

## Date
2025-01-25

## Context

Memory patterns have inherent rhythm and structure:
- Periodic allocation/deallocation cycles
- Usage waves throughout the day
- Process "heartbeats" from regular activity
- Pressure buildups and releases

These patterns map naturally to musical concepts:
- Memory pressure → Intensity/tempo
- Process count → Polyphony
- Allocation rate → Rhythm
- Memory freed → Resolution/release

This creates opportunity for:
- Ambient awareness of system state
- Unique generative art
- Accessibility (audio feedback)
- Novel user experience

## Decision

Implement **Memory Sonification** - real-time generative audio from system memory patterns.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Memory Music System                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐   │
│  │   Memory    │   │   Pattern   │   │      Audio          │   │
│  │   Sampler   │──▶│   Mapper    │──▶│      Engine         │   │
│  │             │   │             │   │                     │   │
│  │ • Usage %   │   │ • Pitch     │   │ • Synthesizer       │   │
│  │ • Alloc/s   │   │ • Tempo     │   │ • Sequencer         │   │
│  │ • Processes │   │ • Timbre    │   │ • Effects           │   │
│  │ • Pressure  │   │ • Density   │   │ • Mixer             │   │
│  └─────────────┘   └─────────────┘   └─────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   Musical Mapping                          │  │
│  │                                                            │  │
│  │   Memory State          →        Musical Expression        │  │
│  │   ─────────────────────────────────────────────────────   │  │
│  │   Low usage (< 30%)     →   Sparse, calm, low notes       │  │
│  │   Normal (30-70%)       →   Flowing, melodic, balanced    │  │
│  │   High (70-85%)         →   Dense, faster, higher pitch   │  │
│  │   Critical (> 85%)      →   Intense, dissonant, urgent    │  │
│  │   Optimization event    →   Resolution chord, sweep       │  │
│  │                                                            │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Sonification Mapping

```rust
pub struct MemoryToMusic {
    /// Memory usage → Base pitch (MIDI note)
    pub pitch_from_usage: fn(f32) -> u8,

    /// Allocation rate → Tempo (BPM)
    pub tempo_from_alloc_rate: fn(f32) -> f32,

    /// Process count → Polyphony (simultaneous notes)
    pub polyphony_from_processes: fn(usize) -> u8,

    /// Memory pressure → Harmonic tension
    pub tension_from_pressure: fn(f32) -> f32,

    /// Optimization → Musical resolution
    pub resolution_from_optimization: fn(&OptimizationResult) -> Resolution,
}

impl Default for MemoryToMusic {
    fn default() -> Self {
        Self {
            pitch_from_usage: |usage| {
                // Low usage = low notes, high usage = high notes
                // C2 (36) to C6 (84)
                36 + (usage * 48.0) as u8
            },
            tempo_from_alloc_rate: |rate| {
                // Low activity = 60 BPM, high activity = 180 BPM
                60.0 + rate.min(1.0) * 120.0
            },
            polyphony_from_processes: |count| {
                // 1-8 simultaneous voices based on process count
                (count / 25).clamp(1, 8) as u8
            },
            tension_from_pressure: |pressure| {
                // 0 = consonant, 1 = dissonant
                pressure.powf(2.0)  // Exponential for dramatic effect
            },
            resolution_from_optimization: |result| {
                if result.freed_mb > 1000.0 {
                    Resolution::Major7Chord
                } else if result.freed_mb > 500.0 {
                    Resolution::MajorChord
                } else {
                    Resolution::FifthInterval
                }
            },
        }
    }
}
```

### Musical Modes

```rust
pub enum MusicMode {
    /// Ambient drone with subtle variations
    Ambient {
        base_note: u8,
        scale: Scale,
        reverb: f32,
    },

    /// Rhythmic patterns from memory activity
    Rhythmic {
        time_signature: (u8, u8),
        swing: f32,
    },

    /// Melodic lines from memory trends
    Melodic {
        scale: Scale,
        melody_length: u8,
    },

    /// Minimal beeps for status awareness
    Minimal {
        only_on_change: bool,
    },

    /// Full orchestral representation
    Orchestral {
        instruments: Vec<Instrument>,
    },
}

pub enum Scale {
    Major,
    Minor,
    Pentatonic,
    Blues,
    Chromatic,
    WholeTone,
}
```

### Audio Engine

```rust
pub struct MemoryMusicEngine {
    sampler: MemorySampler,
    mapper: MemoryToMusic,
    synth: Synthesizer,
    mode: MusicMode,
    output: AudioOutput,
}

impl MemoryMusicEngine {
    /// Start generating music
    pub fn start(&mut self) -> Result<(), AudioError>;

    /// Stop music generation
    pub fn stop(&mut self);

    /// Change musical mode
    pub fn set_mode(&mut self, mode: MusicMode);

    /// Adjust volume
    pub fn set_volume(&mut self, volume: f32);

    /// Get current musical state
    pub fn current_state(&self) -> MusicalState;

    /// Export last N minutes as audio file
    pub fn export(&self, duration: Duration, path: &Path) -> Result<(), Error>;
}

pub struct MusicalState {
    pub current_pitch: u8,
    pub current_tempo: f32,
    pub current_mode: MusicMode,
    pub active_notes: Vec<Note>,
    pub tension_level: f32,
}
```

### Process "Instruments"

Each major process type gets a distinct instrument:

| Process Type | Instrument | Character |
|--------------|------------|-----------|
| Browser | Piano | Melodic, variable |
| IDE/Editor | Strings | Sustained, smooth |
| Terminal | Bass | Rhythmic, punchy |
| Games | Drums | Intense, driving |
| System | Pads | Ambient, constant |
| AI/LLM | Choir | Ethereal, evolving |

### Special Events

| Event | Musical Response |
|-------|------------------|
| Optimization start | Rising arpeggio |
| Memory freed | Resolution chord |
| High pressure | Tension buildup |
| OOM warning | Alarm motif |
| Flow state | Calm melody |
| Game detected | Mode shift to minimal |

### Tray Integration

- **Play/Pause** - Toggle music generation
- **Mode selector** - Choose ambient/rhythmic/melodic
- **Volume slider** - Adjust output level
- **Visualizer** - Optional spectral display

## Consequences

### Positive
- Unique ambient awareness of system state
- Accessibility for visually impaired users
- Generative art from computer usage
- Calming background during work
- Novel product differentiation

### Negative
- Audio processing overhead
- May be annoying if not well-tuned
- Requires audio output
- Subjective quality assessment

### Risks
- User fatigue from repetitive patterns
- Audio conflicts with other apps
- Performance impact during heavy load

## Implementation Phases

### Phase 1: Audio Engine (2 weeks)
- Basic synthesizer
- Audio output setup
- Simple tone generation

### Phase 2: Sonification Mapping (2 weeks)
- Memory to pitch
- Tempo from activity
- Basic musical scales

### Phase 3: Musical Modes (2 weeks)
- Ambient mode
- Rhythmic mode
- Melodic mode

### Phase 4: Polish & UI (1 week)
- Tray integration
- Mode selection
- Volume control
- Export feature

## Success Metrics

| Metric | Target |
|--------|--------|
| Audio Latency | < 50ms |
| CPU Usage | < 2% |
| User Satisfaction | > 70% "pleasant" |
| False Positive Alerts | < 1/hour |

## Technical Requirements

- Audio library: `rodio` or `cpal`
- Synthesis: Custom or `fundsp`
- Sample rate: 44.1kHz
- Buffer size: 512-1024 samples
- Output: System default audio device

## References

- [Sonification Handbook](https://sonification.de/handbook/)
- [Algorithmic Composition](https://mitpress.mit.edu/books/algorithmic-composition)
- [rodio audio library](https://github.com/RustAudio/rodio)
- Existing: `src/monitor/realtime.rs`, `src/windows/memory.rs`
