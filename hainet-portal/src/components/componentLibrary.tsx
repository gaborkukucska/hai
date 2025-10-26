import React from 'react';

// A simple vertical stack component
const Stack: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>{children}</div>
);

// A simple text component
const Text: React.FC<{ children: React.ReactNode }> = ({ children }) => <p>{children}</p>;

// A simple button component
const Button: React.FC<{ children: React.ReactNode; onClick?: () => void }> = ({ children, onClick }) => (
  <button onClick={onClick} style={{ padding: '8px 12px', cursor: 'pointer' }}>
    {children}
  </button>
);

export const componentLibrary: { [key: string]: React.ComponentType<any> } = {
  Stack,
  Text,
  Button,
};
