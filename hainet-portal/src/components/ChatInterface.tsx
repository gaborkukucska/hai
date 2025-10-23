//! # START OF FILE hainet-portal/src/components/ChatInterface.tsx
import { useState, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface ChatMessage {
  id: string
  content: string
  role: 'user' | 'assistant'
  timestamp: number
  attachments?: FileAttachment[]
}

interface FileAttachment {
  name: string
  path: string
  size: number
  mime_type: string
}

interface ChatResponse {
  message: ChatMessage
  agent_state: string
  active_projects: number
}

export default function ChatInterface() {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [attachments, setAttachments] = useState<FileAttachment[]>([])
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Auto-scroll to bottom
  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }

  useEffect(() => {
    scrollToBottom()
  }, [messages])

  // Load message history on mount
  useEffect(() => {
    loadHistory()
  }, [])

  const loadHistory = async () => {
    try {
      const history = await invoke<ChatMessage[]>('get_history')
      setMessages(history)
    } catch (error) {
      console.error('Failed to load history:', error)
    }
  }

  const sendMessage = async () => {
    if (!input.trim() && attachments.length === 0) return

    const userMessage: ChatMessage = {
      id: crypto.randomUUID(),
      content: input,
      role: 'user',
      timestamp: Date.now(),
      attachments: attachments.length > 0 ? attachments : undefined,
    }

    // Add user message immediately for responsive UI
    setMessages(prev => [...prev, userMessage])
    setInput('')
    setAttachments([])
    setIsLoading(true)

    try {
      const response = await invoke<ChatResponse>('send_message', {
        content: input,
        attachments: attachments,
      })

      setMessages(prev => [...prev, response.message])
      console.log('Agent state:', response.agent_state)
      console.log('Active projects:', response.active_projects)
    } catch (error) {
      console.error('Failed to send message:', error)
      setMessages(prev => [...prev, {
        id: crypto.randomUUID(),
        content: `Error: ${error}`,
        role: 'assistant',
        timestamp: Date.now(),
      }])
    } finally {
      setIsLoading(false)
    }
  }

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      sendMessage()
    }
  }

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files
    if (!files) return

    const newAttachments: FileAttachment[] = await Promise.all(
      Array.from(files).map(async (file) => {
        // For web version, we can't access file.path, so we use the file name
        // In Tauri desktop, we can use dialog API to get actual paths
        return {
          name: file.name,
          path: file.name, // Will be resolved by backend if needed
          size: file.size,
          mime_type: file.type || 'application/octet-stream',
        }
      })
    )

    setAttachments(prev => [...prev, ...newAttachments])
  }

  const removeAttachment = (index: number) => {
    setAttachments(prev => prev.filter((_, i) => i !== index))
  }

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  }

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  return (
    <div className="flex flex-col h-full">
      {/* Messages Area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 ? (
          <div className="text-center text-gray-500 py-8">
            <p className="text-lg">Welcome to HAI-Net!</p>
            <p className="text-sm mt-2">Start a conversation with your AI assistant</p>
          </div>
        ) : (
          messages.map((msg) => (
            <div
              key={msg.id}
              className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
            >
              <div
                className={`max-w-[70%] rounded-lg p-3 ${
                  msg.role === 'user'
                    ? 'bg-hai-primary text-white'
                    : 'bg-gray-700 text-gray-100'
                }`}
              >
                <div className="whitespace-pre-wrap break-words">{msg.content}</div>
                {msg.attachments && msg.attachments.length > 0 && (
                  <div className="mt-2 pt-2 border-t border-gray-600 space-y-1">
                    {msg.attachments.map((att, idx) => (
                      <div key={idx} className="text-xs opacity-75">
                        📎 {att.name} ({formatFileSize(att.size)})
                      </div>
                    ))}
                  </div>
                )}
                <div className="text-xs opacity-75 mt-1">
                  {formatTimestamp(msg.timestamp)}
                </div>
              </div>
            </div>
          ))
        )}
        {isLoading && (
          <div className="flex justify-start">
            <div className="bg-gray-700 text-gray-100 rounded-lg p-3">
              <div className="flex items-center space-x-2">
                <div className="animate-pulse">Thinking...</div>
              </div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Attachments Preview */}
      {attachments.length > 0 && (
        <div className="border-t border-gray-700 bg-gray-800 p-2">
          <div className="max-w-4xl mx-auto">
            <div className="text-xs text-gray-400 mb-1">Attachments:</div>
            <div className="flex flex-wrap gap-2">
              {attachments.map((att, idx) => (
                <div
                  key={idx}
                  className="flex items-center gap-2 bg-gray-700 rounded px-3 py-1 text-sm"
                >
                  <span>📎 {att.name}</span>
                  <span className="text-gray-400 text-xs">
                    ({formatFileSize(att.size)})
                  </span>
                  <button
                    onClick={() => removeAttachment(idx)}
                    className="text-red-400 hover:text-red-300 ml-2"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Input Area */}
      <div className="border-t border-gray-700 bg-gray-800 p-4">
        <div className="max-w-4xl mx-auto flex gap-2">
          <input
            ref={fileInputRef}
            type="file"
            multiple
            onChange={handleFileSelect}
            className="hidden"
          />
          <button
            onClick={() => fileInputRef.current?.click()}
            className="bg-gray-700 hover:bg-gray-600 text-white px-4 py-3 rounded-lg transition-colors"
            title="Attach files"
          >
            📎
          </button>
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyPress={handleKeyPress}
            placeholder="Type your message here..."
            disabled={isLoading}
            className="flex-1 bg-gray-700 text-white px-4 py-3 rounded-lg focus:outline-none focus:ring-2 focus:ring-hai-primary disabled:opacity-50"
          />
          <button
            onClick={sendMessage}
            disabled={isLoading || (!input.trim() && attachments.length === 0)}
            className="bg-hai-primary hover:bg-blue-600 text-white px-6 py-3 rounded-lg font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isLoading ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}
