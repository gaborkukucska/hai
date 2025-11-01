// hainet-portal/src/types.ts

export type PrivacyMode = 'Off' | 'Blur';

export interface FrameAnalysisResult {
  objects_detected: string[];
  ocr_text: string;
  gesture: string;
  emotional_valence: number;
}

// Defines an action that can be triggered by a UI component
export interface DynamicUIAction {
  type: 'invoke' | 'submit' | 'callback';
  payload: {
    command: string;
    args?: Record<string, any>;
  };
}

// Represents a single dynamic UI component
export interface DynamicUIComponent {
  type: string;
  props?: Record<string, any>;
  children?: DynamicUIComponent[];
}

// Metrics Types
export interface AgentMetrics {
  agent_type: string;
  total_operations: number;
  success_rate: number;
  avg_response_time_ms: number;
  avg_tokens_used: number;
  json_parse_success_rate: number;
  validation_pass_rate: number;
  syntax_error_rate: number;
  first_operation_unix: number;
  last_operation_unix: number;
}

export interface MetricsSummary {
  total_tasks: number;
  overall_success_rate: number;
  total_tokens: number;
  total_cost_usd: number;
  agents: AgentMetrics[];
  timestamp_unix: number;
}

// Settings Types
export interface Settings {
  theme: string;
  audio_input_device: string | null;
  video_input_device: string | null;
  stt_model: string | null;
  tts_model: string | null;
  vision_model: string | null;
  // Privacy settings
  pii_detection: boolean;
  bias_detection: boolean;
  harm_detection: boolean;
  // Notification settings
  enable_notifications: boolean;
  enable_sound: boolean;
}

export interface DevicePreference {
  device_type: string;  // 'microphone', 'speaker', 'camera'
  device_id: string;
  device_name: string;
  is_default: boolean;
}
