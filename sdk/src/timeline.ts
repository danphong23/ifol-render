// ══════════════════════════════════════════════════
// ifol-render-sdk — Timeline
//
// Manages playback state: current time, play/pause.
// FPS is NOT here — App Layer controls tick rate.
// ══════════════════════════════════════════════════

/**
 * Timeline — shared playback state.
 *
 * SDK owns time. App calls `tick(wallTimestamp)` at whatever
 * frame rate it wants (60fps, 30fps, variable).
 *
 * ```ts
 * const tl = new Timeline(10); // 10 second duration
 * tl.play();
 *
 * // In App's requestAnimationFrame:
 * const time = tl.tick(performance.now());
 * const frame = view.flatten(time);
 * ```
 */
export class Timeline {
  duration: number;
  currentTime = 0;
  playing = false;
  loop = true;

  private playStartWall = 0;
  private playStartTime = 0;

  constructor(duration: number) {
    this.duration = duration;
  }

  play(): void {
    if (this.playing) return;
    this.playing = true;
    this.playStartWall = performance.now();
    this.playStartTime = this.currentTime;
  }

  pause(): void {
    this.playing = false;
  }

  stop(): void {
    this.playing = false;
    this.currentTime = 0;
  }

  seekTo(time: number): void {
    this.currentTime = Math.max(0, Math.min(time, this.duration));
    if (this.playing) {
      this.playStartWall = performance.now();
      this.playStartTime = this.currentTime;
    }
  }

  /**
   * Advance time based on wall clock.
   * Call this each animation frame. Returns the current time.
   *
   * @param wallTimestamp - performance.now() from requestAnimationFrame
   */
  tick(wallTimestamp?: number): number {
    if (this.playing) {
      const wall = wallTimestamp ?? performance.now();
      this.currentTime = this.playStartTime + (wall - this.playStartWall) / 1000;
      if (this.currentTime >= this.duration) {
        if (this.loop) {
          this.currentTime = 0;
          this.playStartTime = 0;
          this.playStartWall = wall;
        } else {
          this.currentTime = this.duration;
          this.playing = false;
        }
      }
    }
    return this.currentTime;
  }
}
