// hainet-portal/src/types.ts

export type PrivacyMode = 'Off' | 'Blur';

export interface FrameAnalysisResult {
  objects_detected: string[];
  ocr_text: string;
  gesture: string;
  emotional_valence: number;
}
