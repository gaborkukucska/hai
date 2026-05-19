import React, { useState } from 'react';

export default function SocialFeed() {
  const [activeTab, setActiveTab] = useState<'global' | 'following'>('global');
  const [postContent, setPostContent] = useState('');
  const [posts, setPosts] = useState([
    {
      id: 1,
      author: 'Satoshi Node',
      time: '10 minutes ago via P2P',
      content: 'Testing the new HAI-Net decentralized feed. The integration with TrippleEffect agents means we can auto-generate content directly into the mesh! 🚀',
    }
  ]);

  const handlePost = () => {
    if (!postContent.trim()) return;
    setPosts([
      {
        id: Date.now(),
        author: 'My Node (You)',
        time: 'Just now',
        content: postContent,
      },
      ...posts
    ]);
    setPostContent('');
  };

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        
        {/* Header */}
        <div className="flex justify-between items-center mb-8">
          <h1 className="text-2xl font-bold">Mesh Social Feed</h1>
          <div className="flex gap-2">
             <button 
               onClick={() => setActiveTab('global')}
               className={`px-3 py-1.5 rounded-md text-sm font-medium ${activeTab === 'global' ? 'bg-theme-bg-tertiary' : 'bg-theme-bg-secondary text-theme-text-muted hover:text-theme-text-primary'}`}
             >Global</button>
             <button 
               onClick={() => setActiveTab('following')}
               className={`px-3 py-1.5 rounded-md text-sm font-medium ${activeTab === 'following' ? 'bg-theme-bg-tertiary' : 'bg-theme-bg-secondary text-theme-text-muted hover:text-theme-text-primary'}`}
             >Following</button>
          </div>
        </div>

        {/* Composer */}
        <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-4">
          <textarea 
            placeholder="Share something with the mesh..." 
            className="w-full bg-transparent resize-none focus:outline-none min-h-[80px] text-theme-text-primary"
            value={postContent}
            onChange={(e) => setPostContent(e.target.value)}
          />
          <div className="flex justify-between items-center mt-2 pt-2 border-t border-theme-border">
            <div className="flex gap-2">
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🖼️</button>
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🎥</button>
            </div>
            <button 
              onClick={handlePost}
              disabled={!postContent.trim()}
              className="px-4 py-1.5 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-full hover:bg-theme-accent-secondary transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed">
              Post to Mesh
            </button>
          </div>
        </div>

        {/* Feed Posts Placeholder */}
        <div className="space-y-4">
          {posts.map(post => (
            <div key={post.id} className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
              <div className="flex items-center gap-3 mb-3">
                 <div className="w-10 h-10 rounded-full bg-theme-bg-tertiary flex items-center justify-center text-lg font-bold">
                   {post.author.charAt(0)}
                 </div>
                 <div>
                   <p className="font-semibold text-sm">{post.author}</p>
                   <p className="text-xs text-theme-text-muted">{post.time}</p>
                 </div>
              </div>
              <p className="text-theme-text-secondary text-sm">
                {post.content}
              </p>
            </div>
          ))}
        </div>

      </div>
    </div>
  );
}
