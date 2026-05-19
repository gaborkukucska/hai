// HTTP Fallback Shim for Tauri IPC
// This allows the React app to communicate with hainet-core across the local network

export async function invoke<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const url = `/api/invoke`;
  
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ cmd, args }),
  });

  if (!response.ok) {
    const errText = await response.text();
    throw new Error(`Invoke failed (${response.status}): ${errText}`);
  }

  // Handle empty responses
  const text = await response.text();
  if (!text) return undefined as any as T;
  
  return JSON.parse(text) as T;
}

// Mock event listener for Tauri events (stubbed for now, can be implemented with Server-Sent Events later)
export type UnlistenFn = () => void;
export async function listen(event: string, handler: (event: any) => void): Promise<UnlistenFn> {
  console.warn(`[Tauri Shim] Event listening not yet implemented for: ${event}`);
  return () => {};
}

// Mock filesystem plugins
export async function save(options: any): Promise<string | null> {
  console.warn("[Tauri Shim] save dialog not supported in web browser");
  return null;
}

export async function writeTextFile(path: string, contents: string): Promise<void> {
  console.warn(`[Tauri Shim] writeTextFile not supported in web browser for path: ${path}`);
}
