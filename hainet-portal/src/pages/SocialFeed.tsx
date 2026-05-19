// <!-- # START OF FILE hainet-portal/src/pages/SocialFeed.tsx -->
// Mesh Social Feed — wired to hainet-social gossip engine via invoke().
// Posts are created via the backend and will eventually broadcast via gossip protocol.

import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

/** Shape of a social post from the backend */
interface Post {
  id: string;
  author: string;
  content: string;
  timestamp: string;
}

export default function SocialFeed() {
  const [activeTab, setActiveTab] = useState<'global' | 'following'>('global');
  const [postContent, setPostContent] = useState('');
  const [posts, setPosts] = useState<Post[]>([]);
  const [isPosting, setIsPosting] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  /** Fetch all posts from the backend social feed */
  const fetchPosts = async () => {
    try {
      const result = await invoke<{ posts: Post[]; total: number }>('get_social_feed');
      if (result?.posts) {
        setPosts(result.posts);
        console.debug('[SocialFeed] Loaded', result.total, 'posts');
      }
    } catch (e: any) {
      console.debug('[SocialFeed] Backend not available:', e.message);
    } finally {
      setIsLoading(false);
    }
  };

  // Load posts on mount and poll every 10 seconds for new gossip
  useEffect(() => {
    fetchPosts();
    const interval = setInterval(fetchPosts, 10000);
    return () => clearInterval(interval);
  }, []);

  /** Create a new post through the backend (which will eventually gossip it) */
  const handlePost = async () => {
    if (!postContent.trim() || isPosting) return;

    setIsPosting(true);
    try {
      const result = await invoke<{ status: string; post: Post }>('create_post', {
        content: postContent,
      });

      if (result?.post) {
        // Optimistically add the post to the top of the list
        setPosts(prev => [result.post, ...prev]);
        setPostContent('');
        console.debug('[SocialFeed] Post created:', result.post.id);
      }
    } catch (e: any) {
      console.error('[SocialFeed] Failed to create post:', e);
    } finally {
      setIsPosting(false);
    }
  };

  /** Format a timestamp into a human-readable relative time */
  const formatTime = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);

      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins} minute${diffMins !== 1 ? 's' : ''} ago`;
      const diffHrs = Math.floor(diffMins / 60);
      if (diffHrs < 24) return `${diffHrs} hour${diffHrs !== 1 ? 's' : ''} ago`;
      return date.toLocaleDateString();
    } catch {
      return timestamp;
    }
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

        {/* Composer — wired to create_post backend endpoint */}
        <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-4">
          <textarea
            id="social-post-input"
            placeholder="Share something with the mesh..."
            className="w-full bg-transparent resize-none focus:outline-none min-h-[80px] text-theme-text-primary"
            value={postContent}
            onChange={(e) => setPostContent(e.target.value)}
            disabled={isPosting}
          />
          <div className="flex justify-between items-center mt-2 pt-2 border-t border-theme-border">
            <div className="flex gap-2">
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🖼️</button>
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🎥</button>
            </div>
            <button
              id="post-to-mesh-btn"
              onClick={handlePost}
              disabled={!postContent.trim() || isPosting}
              className="px-4 py-1.5 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-full hover:bg-theme-accent-secondary transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed">
              {isPosting ? 'Posting...' : 'Post to Mesh'}
            </button>
          </div>
        </div>

        {/* Feed Posts — loaded from backend */}
        <div className="space-y-4">
          {isLoading ? (
            <div className="text-center py-8">
              <p className="text-sm text-theme-text-muted animate-pulse">Loading feed...</p>
            </div>
          ) : posts.length === 0 ? (
            <div className="text-center py-8 bg-theme-bg-secondary border border-theme-border rounded-xl">
              <p className="text-theme-text-muted">No posts yet. Be the first to share with the mesh! 🚀</p>
            </div>
          ) : (
            posts.map(post => (
              <div key={post.id} className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
                <div className="flex items-center gap-3 mb-3">
                   <div className="w-10 h-10 rounded-full bg-theme-bg-tertiary flex items-center justify-center text-lg font-bold">
                     {post.author.charAt(0).toUpperCase()}
                   </div>
                   <div>
                     <p className="font-semibold text-sm">{post.author}</p>
                     <p className="text-xs text-theme-text-muted">{formatTime(post.timestamp)} via P2P</p>
                   </div>
                </div>
                <p className="text-theme-text-secondary text-sm whitespace-pre-wrap">
                  {post.content}
                </p>
              </div>
            ))
          )}
        </div>

      </div>
    </div>
  );
}
