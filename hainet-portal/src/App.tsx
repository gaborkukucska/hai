//! # START OF FILE hainet-portal/src/App.tsx
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import ChatInterface from './components/ChatInterface';
import { BottomNavigation } from './components/BottomNavigation';
import MetricsDashboard from './pages/MetricsDashboard';
import Settings from './pages/Settings';

function App() {
  return (
    <Router>
      <div className="flex flex-col h-screen bg-gray-900">
        {/* Header */}
        <header className="bg-gray-800 border-b border-gray-700 p-4 flex-shrink-0">
          <h1 className="text-2xl font-bold text-hai-primary">HAI-Net Portal</h1>
          <p className="text-sm text-gray-400">Multimodal AI Interface</p>
        </header>

        {/* Main Content */}
        <main className="flex-1 flex flex-col overflow-hidden pb-16">
          <Routes>
            <Route path="/" element={<ChatInterface />} />
            <Route path="/metrics" element={<MetricsDashboard />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </main>

        {/* Bottom Navigation */}
        <BottomNavigation />
      </div>
    </Router>
  )
}

export default App
