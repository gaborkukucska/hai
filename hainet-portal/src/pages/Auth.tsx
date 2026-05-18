import React, { useState, useEffect } from 'react';
import { AlertCircle, CheckCircle2, Copy, RefreshCw, KeyRound, ShieldAlert } from 'lucide-react';

type AuthState = 'checking' | 'setup' | 'login' | 'error';

export default function Auth() {
  const [authState, setAuthState] = useState<AuthState>('checking');
  
  // Setup Wizard State
  const [seedPhrase, setSeedPhrase] = useState<string[]>([]);
  const [appPassphrase, setAppPassphrase] = useState('');
  const [confirmPassphrase, setConfirmPassphrase] = useState('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [copied, setCopied] = useState(false);
  
  // Login State
  const [loginPassphrase, setLoginPassphrase] = useState('');
  const [errorMsg, setErrorMsg] = useState('');

  // 1. Initial Status Check
  useEffect(() => {
    checkStatus();
  }, []);

  const checkStatus = async () => {
    try {
      const res = await fetch('/api/auth/status');
      if (res.ok) {
        const data = await res.json();
        if (data.status === 'setup_required') {
          setAuthState('setup');
          generateSeed();
        } else {
          setAuthState('login');
        }
      } else {
        setAuthState('error');
      }
    } catch (e) {
      // If we can't reach the backend, we might be in Vite dev mode without the Rust backend
      // Fallback for UI testing
      console.warn("Backend not reachable. Falling back to setup UI for testing.");
      setAuthState('setup');
      generateSeed();
    }
  };

  const generateSeed = async () => {
    setIsGenerating(true);
    try {
      const res = await fetch('/api/auth/generate-seed');
      if (res.ok) {
        const data = await res.json();
        setSeedPhrase(data.seed_phrase.split(' '));
      } else {
        // Fallback mock seed
        setSeedPhrase("abandon ability able about above absent absorb abstract absurd abuse access accident account accuse achieve acid acoustic acquire across act action actor actress actual".split(' '));
      }
    } catch (e) {
       setSeedPhrase("abandon ability able about above absent absorb abstract absurd abuse access accident account accuse achieve acid acoustic acquire across act action actor actress actual".split(' '));
    } finally {
      setIsGenerating(false);
      setCopied(false);
    }
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(seedPhrase.join(' '));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleSetupSubmit = async () => {
    if (appPassphrase.length < 8) {
      setErrorMsg('App Passphrase must be at least 8 characters.');
      return;
    }
    if (appPassphrase !== confirmPassphrase) {
      setErrorMsg('Passphrases do not match.');
      return;
    }

    try {
      const res = await fetch('/api/auth/setup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          seed_phrase: seedPhrase.join(' '),
          app_passphrase: appPassphrase
        })
      });

      if (res.ok) {
        // Setup complete, switch to login
        setAuthState('login');
        setErrorMsg('');
      } else {
        const err = await res.text();
        setErrorMsg(err);
      }
    } catch (e) {
      setErrorMsg('Network error saving configuration.');
    }
  };

  const handleLoginSubmit = async () => {
    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ app_passphrase: loginPassphrase })
      });

      if (res.ok) {
        // Login successful, reload to apply cookie / state
        window.location.reload();
      } else {
        setErrorMsg('Invalid App Passphrase.');
      }
    } catch (e) {
      setErrorMsg('Network error connecting to node.');
    }
  };

  if (authState === 'checking') {
    return <div className="h-screen flex items-center justify-center bg-theme-bg-primary text-theme-text-primary">Loading Node Profile...</div>;
  }

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-theme-bg-primary text-theme-text-primary p-4 overflow-y-auto">
      <div className="max-w-2xl w-full bg-theme-bg-secondary border border-theme-border rounded-xl shadow-xl overflow-hidden my-8">
        
        {/* Header */}
        <div className="p-8 text-center border-b border-theme-border bg-theme-bg-tertiary/20">
          <div className="w-16 h-16 bg-theme-accent-primary/10 rounded-2xl flex items-center justify-center mx-auto mb-4 border border-theme-accent-primary/20">
            <ShieldAlert size={32} className="text-theme-accent-primary" />
          </div>
          <h1 className="text-3xl font-bold text-theme-text-primary mb-2">HAI-Net Portal</h1>
          <p className="text-theme-text-muted">Decentralized P2P Mesh Network</p>
        </div>

        {/* Dynamic Body */}
        <div className="p-8 space-y-6">
          {errorMsg && (
            <div className="bg-theme-accent-danger/10 border border-theme-accent-danger/50 text-theme-accent-danger px-4 py-3 rounded flex items-center gap-2">
              <AlertCircle size={18} />
              <p className="text-sm">{errorMsg}</p>
            </div>
          )}

          {authState === 'login' && (
            <div className="space-y-6 max-w-sm mx-auto">
              <div className="text-center mb-8">
                <h2 className="text-xl font-semibold">Node Locked</h2>
                <p className="text-sm text-theme-text-secondary mt-1">Enter your App Passphrase to unlock.</p>
              </div>
              
              <div>
                <input 
                  type="password" 
                  id="login-passphrase"
                  name="login-passphrase"
                  placeholder="App Passphrase"
                  value={loginPassphrase}
                  onChange={(e) => setLoginPassphrase(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleLoginSubmit()}
                  className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-3 text-theme-text-primary focus:outline-none focus:border-theme-accent-primary text-center"
                />
              </div>
              
              <button 
                onClick={handleLoginSubmit}
                className="w-full bg-theme-accent-primary hover:bg-theme-accent-secondary text-theme-bg-primary font-bold py-3 px-4 rounded-md transition-colors flex justify-center items-center gap-2"
              >
                <KeyRound size={18} /> Unlock Node
              </button>
            </div>
          )}

          {authState === 'setup' && (
            <div className="space-y-8">
              <div className="text-center">
                <h2 className="text-2xl font-semibold text-theme-accent-primary">Master Seed Phrase</h2>
                <p className="text-sm text-theme-text-secondary mt-2 max-w-lg mx-auto">
                  This 24-word phrase is the cryptographic master key for your identity. 
                  Write it down and store it in multiple secure physical locations.
                </p>
                <div className="mt-4 p-3 bg-theme-accent-danger/10 border border-theme-accent-danger/30 rounded-lg text-theme-accent-danger text-sm font-bold animate-pulse inline-block">
                  DO NOT SAVE AS A PHOTO ON YOUR PHONE! 😂
                </div>
              </div>

              <div className="bg-theme-bg-primary border border-theme-border rounded-lg p-6 relative">
                <div className="grid grid-cols-3 sm:grid-cols-4 gap-4">
                  {seedPhrase.map((word, idx) => (
                    <div key={idx} className="flex gap-2 items-center bg-theme-bg-tertiary/50 px-3 py-2 rounded">
                      <span className="text-xs text-theme-text-muted w-4 text-right select-none">{idx + 1}.</span>
                      <span className="font-mono text-theme-text-primary font-medium">{word}</span>
                    </div>
                  ))}
                </div>
                
                <div className="flex justify-between items-center mt-6 pt-4 border-t border-theme-border">
                  <button 
                    onClick={generateSeed}
                    disabled={isGenerating}
                    className="flex items-center gap-2 text-sm text-theme-text-secondary hover:text-theme-accent-primary transition-colors"
                  >
                    <RefreshCw size={16} className={isGenerating ? 'animate-spin' : ''} />
                    Regenerate
                  </button>
                  <button 
                    onClick={copyToClipboard}
                    className="flex items-center gap-2 text-sm text-theme-text-secondary hover:text-theme-text-primary transition-colors bg-theme-bg-tertiary px-3 py-1.5 rounded"
                  >
                    {copied ? <CheckCircle2 size={16} className="text-theme-accent-success" /> : <Copy size={16} />}
                    {copied ? 'Copied' : 'Copy to Clipboard'}
                  </button>
                </div>
              </div>

              <div className="border-t border-theme-border pt-8 space-y-4 max-w-md mx-auto">
                <div className="text-center mb-6">
                  <h3 className="text-lg font-semibold">Create App Passphrase</h3>
                  <p className="text-xs text-theme-text-muted mt-1">
                    A simpler password (min 8 chars) to encrypt your 24 words and unlock this UI daily.
                  </p>
                </div>

                <div>
                  <input 
                    type="password" 
                    id="setup-app-passphrase"
                    name="setup-app-passphrase"
                    placeholder="New App Passphrase"
                    value={appPassphrase}
                    onChange={(e) => setAppPassphrase(e.target.value)}
                    className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 text-theme-text-primary focus:outline-none focus:border-theme-accent-primary"
                  />
                </div>
                <div>
                  <input 
                    type="password" 
                    id="setup-confirm-passphrase"
                    name="setup-confirm-passphrase"
                    placeholder="Confirm App Passphrase"
                    value={confirmPassphrase}
                    onChange={(e) => setConfirmPassphrase(e.target.value)}
                    className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 text-theme-text-primary focus:outline-none focus:border-theme-accent-primary"
                  />
                </div>
                
                <button 
                  onClick={handleSetupSubmit}
                  disabled={!appPassphrase || !confirmPassphrase}
                  className="w-full bg-theme-accent-primary hover:bg-theme-accent-secondary text-theme-bg-primary font-bold py-3 px-4 rounded-md transition-colors mt-4 disabled:opacity-50"
                >
                  Encrypt & Secure Node
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
