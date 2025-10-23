//! # START OF FILE hainet-portal/src/App.tsx
import ChatInterface from './components/ChatInterface'

function App() {
  return (
    <div className="flex flex-col h-screen bg-gray-900">
      {/* Header */}
      <header className="bg-gray-800 border-b border-gray-700 p-4 flex-shrink-0">
        <h1 className="text-2xl font-bold text-hai-primary">HAI-Net Portal</h1>
        <p className="text-sm text-gray-400">Multimodal AI Interface</p>
      </header>

      {/* Main Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        <ChatInterface />
      </main>
    </div>
  )
}

export default App
