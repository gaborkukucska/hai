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

export interface TrendDataPoint {
  timestamp: number;
  operations: number;
  success_rate: number;
  avg_latency_ms: number;
}

export type TrendInterval = 'Hourly' | 'Daily' | 'Weekly';

export interface TimeRange {
  start?: number; // Unix timestamp
  end?: number;   // Unix timestamp
  interval?: TrendInterval;
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

// Model Preference Types
export interface ModelPreference {
  agent_type: 'Admin' | 'PM' | 'Worker';
  preferred_family: string;
  allow_fallback: boolean;
}

export interface ModelFamily {
  id: string;
  name: string;
  description: string;
}

// Predefined model families
export const MODEL_FAMILIES: ModelFamily[] = [
  { id: 'auto', name: 'Auto (Best Available)', description: 'Automatically select best model' },
  { id: 'llama3', name: 'Llama 3', description: 'Meta\'s Llama 3 family' },
  { id: 'gemma3', name: 'Gemma 3', description: 'Google\'s Gemma 3 family' },
  { id: 'qwen', name: 'Qwen', description: 'Alibaba\'s Qwen family' },
  { id: 'deepseek', name: 'DeepSeek', description: 'DeepSeek family' },
  { id: 'phi', name: 'Phi', description: 'Microsoft\'s Phi family' },
];

// Agent Types
export interface AgentId {
  type: string;
  name: string;
}

export interface AgentStatus {
  state: string;
  activity: string;
  last_updated: number;
}

export interface AgentInfo {
  id: AgentId;
  status?: AgentStatus;
  instance_id: string;
  domain?: string;
  worker_type?: string;
}

export interface TaskInfo {
  id: string;
  title: string;
  status: string;
}

export interface ProjectInfo {
  id: string;
  title: string;
  status: string;
  tasks: TaskInfo[];
}
