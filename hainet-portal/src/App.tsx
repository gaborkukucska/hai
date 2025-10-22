//! # START OF FILE hainet-portal/src/App.tsx
import { useState } from 'react'

function App() {
  const [message, setMessage] = useState('')

  return (
    <div className="flex flex-col h-full bg-gray-900">
      {/* Header */}
      <header className="bg-gray-800 border-b border-gray-700 p-4">
        <h1 className="text-2xl font-bold text-hai-primary">HAI-Net Portal</h1>
        <p className="text-sm text-gray-400">Multimodal AI Interface</p>
      </header>

      {/* Main Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Chat Messages Area */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          <div className="text-center text-gray-500 py-8">
            <p className="text-lg">Welcome to HAI-Net!</p>
            <p className="text-sm mt-2">Start a conversation with your AI assistant</p>
          </div>
        </div>

        {/* Input Area */}
        <div className="border-t border-gray-700 bg-gray-800 p-4">
          <div className="max-w-4xl mx-auto flex gap-2">
            <input
              type="text"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              placeholder="Type your message here..."
              className="flex-1 bg-gray-700 text-white px-4 py-3 rounded-lg focus:outline-none focus:ring-2 focus:ring-hai-primary"
              onKeyPress={(e) => {
                if (e.key === 'Enter') {
                  console.log('Send message:', message)
                  setMessage('')
                }
              }}
            />
            <button
              onClick={() => {
                console.log('Send message:', message)
                setMessage('')
              }}
              className="bg-hai-primary hover:bg-blue-600 text-white px-6 py-3 rounded-lg font-medium transition-colors"
            >
              Send
            </button>
          </div>
        </div>
      </main>
    </div>
  )
}

export default App
