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
  // The type of the component to render (e.g., 'Stack', 'Text', 'Button', 'Input')
  type: string;
  // Properties to pass to the React component (e.g., { text: 'Click me' })
  props?: Record<string, any>;
  // Child components or text content
  children?: (DynamicUIComponent | string)[];
  // Action to perform on interaction (e.g., for a button click)
  action?: DynamicUIAction;
}
