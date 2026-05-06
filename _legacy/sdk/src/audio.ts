// ══════════════════════════════════════════════════
// ifol-render-sdk — Audio Model
//
// Manages audio tracks and clips for parallel export.
// Counterpart to Scene (visuals).
// ══════════════════════════════════════════════════

import type { AudioEntity } from './types.js';
import type { Scene } from './scene.js';

/** Describes the JSON format expected by the backend / CLI for 'audio_clips' */
export interface FlatAudioClip {
  path: string;
  start_time: number;
  duration?: number;
  volume: number;
  fade_in: number;
  fade_out: number;
  offset: number;
}

/**
 * AudioTrack groups related AudioEntities (e.g. 'bgm', 'sfx', 'dialogue').
 */
export class AudioTrack {
  id: string;
  clips: AudioEntity[] = [];
  volume: number = 1.0;
  muted: boolean = false;

  constructor(id: string) {
    this.id = id;
  }

  addClip(clip: AudioEntity) {
    this.clips.push(clip);
  }

  removeClip(id: string) {
    this.clips = this.clips.filter(c => c.id !== id);
  }
}

/**
 * AudioScene — the timeline model for all audio.
 * Manages Tracks, which in turn manage AudioEntities.
 */
export class AudioScene {
  tracks: Map<string, AudioTrack> = new Map();
  private nextId = 1;

  constructor() {}

  getTrack(id: string): AudioTrack {
    if (!this.tracks.has(id)) {
      this.tracks.set(id, new AudioTrack(id));
    }
    return this.tracks.get(id)!;
  }

  /**
   * Adds an audio clip to a track.
   * If partial object is given, fills with defaults.
   */
  addClip(params: Partial<AudioEntity> & { source: string }): AudioEntity {
    const id = params.id || `audio_${this.nextId++}`;
    const trackId = params.trackId || 'default';
    
    const clip: AudioEntity = {
      id,
      label: params.label || id,
      source: params.source,
      startTime: params.startTime ?? 0,
      duration: params.duration,
      offset: params.offset ?? 0,
      volume: params.volume ?? 1.0,
      fadeIn: params.fadeIn ?? 0,
      fadeOut: params.fadeOut ?? 0,
      trackId,
    };

    this.getTrack(trackId).addClip(clip);
    return clip;
  }

  removeClip(id: string) {
    for (const track of this.tracks.values()) {
      track.removeClip(id);
    }
  }

  allClips(): AudioEntity[] {
    const all: AudioEntity[] = [];
    for (const track of this.tracks.values()) {
      all.push(...track.clips);
    }
    return all;
  }

  /**
   * Scans a visual Scene and automatically extracts audio clips from Video entities.
   * They are placed in a special 'video_audio' track.
   */
  autoExtractVideoAudio(scene: Scene) {
    const track = this.getTrack('video_audio');
    // Clear existing auto-extracted video clips to prevent duplicates
    track.clips = [];
    
    // We use a Set to prevent same source playing twice if duplicated (though user might want instances)
    // For now, let's allow instances if they are at different times.
    for (const e of scene.allEntities()) {
      if (e.type === 'video' && e.source) {
        track.addClip({
          id: `vid_audio_${e.id}`,
          label: `Audio (${e.label || e.id})`,
          source: e.sourcePath || e.source,
          startTime: e.startTime,
          duration: e.duration,
          offset: 0,
          volume: 1.0,
          fadeIn: 0,
          fadeOut: 0,
          trackId: 'video_audio',
        });
      }
    }
  }

  /**
   * Generates the flat JSON array expected by the CLI export endpoint.
   */
  flattenForExport(): FlatAudioClip[] {
    const result: FlatAudioClip[] = [];
    for (const track of this.tracks.values()) {
      if (track.muted) continue;
      
      for (const clip of track.clips) {
        // Multiply clip volume by track volume
        const finalVolume = clip.volume * track.volume;
        if (finalVolume <= 0) continue;

        result.push({
          path: clip.source,
          start_time: clip.startTime,
          duration: clip.duration,
          volume: finalVolume,
          fade_in: clip.fadeIn,
          fade_out: clip.fadeOut,
          offset: clip.offset,
        });
      }
    }
    return result;
  }
}
