import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { Sidebar } from './components/Sidebar';
import Auth from './pages/Auth';
import ChatView from './pages/ChatView';
import SocialFeed from './pages/SocialFeed';
import AgentStudio from './pages/AgentStudio';
import ComputeNode from './pages/ComputeNode';
import NetworkSettings from './pages/NetworkSettings';
import Settings from './pages/Settings';
import UserProfile from './pages/UserProfile';
import { LogOverlay } from './components/LogOverlay';
import { useState, useEffect } from 'react';

function App() {
  const [isAuthenticated, setIsAuthenticated] = useState<boolean | null>(null);

  useEffect(() => {
    const verifyAuth = async () => {
      try {
        const res = await fetch('/api/auth/verify');
        if (res.ok) {
          setIsAuthenticated(true);
        } else {
          setIsAuthenticated(false);
        }
      } catch (e) {
        setIsAuthenticated(false);
      }
    };
    verifyAuth();
  }, []);

  if (isAuthenticated === null) {
    return <div className="h-screen bg-theme-bg-primary text-theme-text-primary flex items-center justify-center">Verifying session...</div>;
  }

  return (
    <Router>
      <div className="flex h-screen bg-theme-bg-primary overflow-hidden font-sans text-theme-text-primary">

        {!isAuthenticated ? (
          <div className="flex-1 w-full h-full">
            <Routes>
              <Route path="*" element={<Auth />} />
            </Routes>
          </div>
        ) : (
          <>
            <Sidebar />
            <main className="flex-1 flex flex-col overflow-hidden relative">
              <Routes>
                <Route path="/" element={<Navigate to="/chat" replace />} />
                <Route path="/chat" element={<ChatView />} />
                <Route path="/feed" element={<SocialFeed />} />
                <Route path="/studio" element={<AgentStudio />} />
                <Route path="/compute" element={<ComputeNode />} />
                <Route path="/network" element={<NetworkSettings />} />
                <Route path="/settings" element={<Settings />} />
                <Route path="/profile" element={<UserProfile />} />
                <Route path="*" element={<Navigate to="/chat" replace />} />
              </Routes>
            </main>
            <LogOverlay />
          </>
        )}
      </div>
    </Router>
  );
}

export default App;
