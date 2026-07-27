/**
 * The window as browsers that predate the unprefixed constructor expose it.
 *
 * Safari shipped `webkitAudioContext` for years and iOS kept it long after; the DOM lib has no
 * declaration for it because it is not standard. Naming it here is the difference between "this
 * property may not exist" and `any`, which also silently permitted calling it with anything.
 */
export interface LegacyAudioWindow extends Window {
  webkitAudioContext?: typeof AudioContext;
}

/**
 * The parts of a panner the spatial update path reads.
 *
 * Structural rather than `PannerNode`, because the cache legitimately holds two different things.
 * A modern node positions through `AudioParam`s; a pre-`AudioParam` browser (Safari, older Chrome)
 * only has the deprecated `setPosition`, which the current DOM lib no longer declares at all. Under
 * jsdom there is a third: a stand-in, because jsdom implements no Web Audio whatsoever, so the
 * legacy branch cannot be reached through the real API. Naming the union structurally is what lets
 * a test put a legacy-shaped node in the cache without casting its way past the type system.
 */
export interface SpatialPanner {
  positionX?: AudioParam;
  positionY?: AudioParam;
  positionZ?: AudioParam;
  /** Pre-`AudioParam` browsers only. */
  setPosition?(x: number, y: number, z: number): void;
}

/**
 * The listener's pre-`AudioParam` API.
 *
 * Same deprecation as `SpatialPanner.setPosition`, but the listener is never injected — it is
 * always `ctx.listener` — so this stays a plain widening of the real type rather than a structural
 * stand-in.
 */
interface LegacySpatialListener extends AudioListener {
  setPosition(x: number, y: number, z: number): void;
  setOrientation(
    forwardX: number,
    forwardY: number,
    forwardZ: number,
    upX: number,
    upY: number,
    upZ: number,
  ): void;
}

export class AudioManager {
  public ctx: AudioContext | null = null;
  public masterGain: GainNode | null = null;
  private currentVolume: number = 1.0;
  private isMuted: boolean = false;
  /**
   * Spatial sources by id.
   *
   * Public because the legacy-`setPosition` branch of `updateSpatialSource` has no other way to be
   * reached under jsdom, which implements no Web Audio: a test seeds this map with a legacy-shaped
   * node. The alternative was a test that cast the manager to `any` to reach a private field, which
   * is the same access with the type checking removed.
   */
  public readonly panners: Map<string, SpatialPanner> = new Map();

  // Procedural wind synthesizer elements
  private windGain: GainNode | null = null;
  private windFilter: BiquadFilterNode | null = null;
  private currentWeather: string = 'clear';
  private currentSpeed: number = 1.0;

  constructor() {
    // Deferred initialization
  }

  public initialize(): void {
    if (this.ctx) return;

    const AudioContextClass = window.AudioContext || (window as LegacyAudioWindow).webkitAudioContext;
    if (AudioContextClass) {
      try {
        this.ctx = new AudioContextClass();
        this.masterGain = this.ctx.createGain();
        this.masterGain.connect(this.ctx.destination);
        this.masterGain.gain.value = this.isMuted ? 0 : this.currentVolume;

        // 1. White noise buffer generation
        const bs = this.ctx.sampleRate * 2;
        const buf = this.ctx.createBuffer(1, bs, this.ctx.sampleRate);
        const d = buf.getChannelData(0);
        for (let i = 0; i < bs; i++) {
          d[i] = Math.random() * 2 - 1;
        }

        // 2. Buffer source setup
        const src = this.ctx.createBufferSource();
        src.buffer = buf;
        src.loop = true;

        // 3. Lowpass filter at 400Hz
        this.windFilter = this.ctx.createBiquadFilter();
        this.windFilter.type = 'lowpass';
        this.windFilter.frequency.value = 400 + this.currentSpeed * 50;

        // 4. Wind gain Node
        this.windGain = this.ctx.createGain();
        const isRainOrSnow = this.currentWeather === 'rain' || this.currentWeather === 'snow';
        this.windGain.gain.value = this.currentVolume * (isRainOrSnow ? 0.18 : 0.08);

        // 5. Connect node chain
        src.connect(this.windFilter).connect(this.windGain).connect(this.masterGain);
        src.start();
      } catch (e) {
        console.error("Failed to initialize AudioContext", e);
      }
    }
  }

  public mute(): void {
    this.isMuted = true;
    if (this.masterGain) {
      this.masterGain.gain.setValueAtTime(0, this.ctx?.currentTime || 0);
    }
  }

  public unmute(): void {
    this.isMuted = false;
    if (this.masterGain) {
      this.masterGain.gain.setValueAtTime(this.currentVolume, this.ctx?.currentTime || 0);
    }
  }

  public getIsMuted(): boolean {
    return this.isMuted;
  }

  public getVolume(): number {
    return this.currentVolume;
  }

  public setVolume(vol: number): void {
    this.currentVolume = Math.max(0, Math.min(1, vol));
    if (this.masterGain && !this.isMuted) {
      this.masterGain.gain.setValueAtTime(this.currentVolume, this.ctx?.currentTime || 0);
    }
    this.updateWindGain();
  }

  private updateWindGain(): void {
    if (this.ctx && this.windGain) {
      const isRainOrSnow = this.currentWeather === 'rain' || this.currentWeather === 'snow';
      const targetGain = this.currentVolume * (isRainOrSnow ? 0.18 : 0.08);
      this.windGain.gain.setValueAtTime(targetGain, this.ctx.currentTime);
    }
  }

  public updateEnvironment(weather: string, speed: number, volume: number): void {
    this.currentWeather = weather;
    this.currentSpeed = speed;
    this.setVolume(volume);

    if (this.ctx) {
      this.updateWindGain();
      if (this.windFilter) {
        this.windFilter.frequency.setValueAtTime(400 + speed * 50, this.ctx.currentTime);
      }
    }
  }

  public createSpatialSource(id: string): SpatialPanner | null {
    this.initialize();
    if (!this.ctx) return null;

    const cached = this.panners.get(id);
    if (cached) return cached;

    const panner = this.ctx.createPanner();
    panner.panningModel = 'HRTF';
    panner.connect(this.masterGain || this.ctx.destination);
    
    // Connect a synthesized sound source to the panner
    this.connectSynthesizer(id, panner);

    this.panners.set(id, panner);
    return panner;
  }

  private connectSynthesizer(id: string, panner: PannerNode): void {
    if (!this.ctx) return;
    try {
      if (id === 'waterfall') {
        // Rushing water white noise for the waterfall
        const bs = this.ctx.sampleRate * 2;
        const buf = this.ctx.createBuffer(1, bs, this.ctx.sampleRate);
        const d = buf.getChannelData(0);
        for (let i = 0; i < bs; i++) {
          d[i] = Math.random() * 2 - 1;
        }
        const src = this.ctx.createBufferSource();
        src.buffer = buf;
        src.loop = true;

        const filter = this.ctx.createBiquadFilter();
        filter.type = 'lowpass';
        filter.frequency.value = 600;

        src.connect(filter).connect(panner);
        src.start();
      } else if (id === 'ambient-forest') {
        // Low frequency forest hum
        const osc = this.ctx.createOscillator();
        osc.type = 'sine';
        osc.frequency.value = 150;

        const gain = this.ctx.createGain();
        gain.gain.value = 0.3;

        osc.connect(gain).connect(panner);
        osc.start();
      } else {
        // Fallback generator for test-src or other dynamically created spatial sources
        const osc = this.ctx.createOscillator();
        osc.type = 'sine';
        osc.frequency.value = 440;

        const gain = this.ctx.createGain();
        gain.gain.value = 0.1;

        osc.connect(gain).connect(panner);
        osc.start();
      }
    } catch (err) {
      console.error(`Failed to connect synthesizer for ${id}`, err);
    }
  }

  public updateSpatialSource(id: string, x: number, y: number, z: number): void {
    const panner = this.panners.get(id) || this.createSpatialSource(id);
    if (panner && this.ctx) {
      const time = this.ctx.currentTime;
      const { positionX, positionY, positionZ } = panner;
      if (positionX?.setValueAtTime && positionY && positionZ) {
        positionX.setValueAtTime(x, time);
        positionY.setValueAtTime(y, time);
        positionZ.setValueAtTime(z, time);
      } else {
        panner.setPosition?.(x, y, z);
      }
    }
  }

  public updateListener(
    x: number, y: number, z: number,
    forwardX = 0, forwardY = 0, forwardZ = -1,
    upX = 0, upY = 1, upZ = 0
  ): void {
    this.initialize();
    if (!this.ctx) return;

    const listener = this.ctx.listener;
    const time = this.ctx.currentTime;
    if (listener.positionX && listener.positionX.setValueAtTime) {
      listener.positionX.setValueAtTime(x, time);
      listener.positionY.setValueAtTime(y, time);
      listener.positionZ.setValueAtTime(z, time);
      listener.forwardX.setValueAtTime(forwardX, time);
      listener.forwardY.setValueAtTime(forwardY, time);
      listener.forwardZ.setValueAtTime(forwardZ, time);
      listener.upX.setValueAtTime(upX, time);
      listener.upY.setValueAtTime(upY, time);
      listener.upZ.setValueAtTime(upZ, time);
    } else {
      const legacy = listener as LegacySpatialListener;
      legacy.setPosition(x, y, z);
      legacy.setOrientation(forwardX, forwardY, forwardZ, upX, upY, upZ);
    }
  }
}

export const audioManager = new AudioManager();
