// ElevenLabs Integration for Coucou Voice Assistant
import { ElevenLabsClient } from "@elevenlabs/elevenlabs-js";

export const ELEVENLABS_VOICE_ID = "MClEFoImJXBTgLwdLI5n"; // Soft girl voice agent requested by user
export const ELEVENLABS_FALLBACK_VOICE_ID = "pFZP5JQG7iQjIQuC4Bku"; // Lily (soft, velvety female) for free tier fallback

/**
 * Creates and exports the ElevenLabs client.
 * The client automatically reads ELEVENLABS_API_KEY from the environment.
 */
export function getElevenLabsClient(): ElevenLabsClient {
  return new ElevenLabsClient();
}

export const elevenlabs = getElevenLabsClient();
