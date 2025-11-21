import { useState, useEffect, useRef } from 'react';
import { invoke } from '../lib/tauri';
import { open, save } from '@tauri-apps/plugin-dialog';
import { AgentInfo, ProjectInfo } from '../types';

// Export/Import types
interface ExportMetadata {
    project_id: string;
    project_title: string;
    export_date: string;
    file_count: number;
    total_size: number;
}

interface ImportResult {
    project_id: string;
    original_title: string;
    imported_title: string;
    task_count: number;
    file_count: number;
}

export default function ActiveAgentsList() {
    const [agents, setAgents] = useState<AgentInfo[]>([]);
    const [projects, setProjects] = useState<ProjectInfo[]>([]);
    const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set());
    const [activeMenu, setActiveMenu] = useState<string | null>(null);
    const [showRenameDialog, setShowRenameDialog] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState('');
    const [showConfirmDialog, setShowConfirmDialog] = useState<{
        projectId: string;
        action: 'stop' | 'delete';
    } | null>(null);
    const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);

    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const fetchData = async () => {
            try {
                const [activeAgents, activeProjects] = await Promise.all([
                    invoke<AgentInfo[]>('get_active_agents'),
                    invoke<ProjectInfo[]>('get_active_projects')
                ]);
                setAgents(activeAgents);
                setProjects(activeProjects);
            } catch (error) {
                console.error('Failed to fetch data:', error);
            }
        };

        fetchData();
        const interval = setInterval(fetchData, 2000); // Refresh every 2s
        return () => clearInterval(interval);
    }, []);

    // Close menu when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
                setActiveMenu(null);
            }
        };

        if (activeMenu) {
            document.addEventListener('mousedown', handleClickOutside);
            return () => document.removeEventListener('mousedown', handleClickOutside);
        }
    }, [activeMenu]);

    // Auto-hide toast after 3 seconds
    useEffect(() => {
        if (toast) {
            const timer = setTimeout(() => setToast(null), 3000);
            return () => clearTimeout(timer);
        }
    }, [toast]);

    const toggleProject = (projectId: string) => {
        const newExpanded = new Set(expandedProjects);
        if (newExpanded.has(projectId)) {
            newExpanded.delete(projectId);
        } else {
            newExpanded.add(projectId);
        }
        setExpandedProjects(newExpanded);
    };

    const toggleMenu = (projectId: string, event: React.MouseEvent) => {
        event.stopPropagation();
        setActiveMenu(activeMenu === projectId ? null : projectId);
    };

    const showToast = (message: string, type: 'success' | 'error') => {
        setToast({ message, type });
    };

    const handlePause = async (projectId: string) => {
        try {
            await invoke('pause_project', { projectId });
            showToast('Project paused successfully', 'success');
            setActiveMenu(null);
        } catch (error) {
            showToast(`Failed to pause project: ${error}`, 'error');
        }
    };

    const handleResume = async (projectId: string) => {
        try {
            await invoke('resume_project', { projectId });
            showToast('Project resumed successfully', 'success');
            setActiveMenu(null);
        } catch (error) {
            showToast(`Failed to resume project: ${error}`, 'error');
        }
    };

    const handleStop = (projectId: string) => {
        setShowConfirmDialog({ projectId, action: 'stop' });
        setActiveMenu(null);
    };

    const handleDelete = (projectId: string) => {
        setShowConfirmDialog({ projectId, action: 'delete' });
        setActiveMenu(null);
    };

    const confirmAction = async () => {
        if (!showConfirmDialog) return;

        try {
            if (showConfirmDialog.action === 'stop') {
                await invoke('stop_project', { projectId: showConfirmDialog.projectId });
                showToast('Project stopped successfully', 'success');
            } else {
                await invoke('delete_project', { projectId: showConfirmDialog.projectId });
                showToast('Project deleted successfully', 'success');
            }
        } catch (error) {
            showToast(`Failed to ${showConfirmDialog.action} project: ${error}`, 'error');
        } finally {
            setShowConfirmDialog(null);
        }
    };

    const handleRename = (projectId: string) => {
        const project = projects.find(p => p.id === projectId);
        if (project) {
            setRenameValue(project.title);
            setShowRenameDialog(projectId);
            setActiveMenu(null);
        }
    };

    const confirmRename = async () => {
        if (!showRenameDialog || !renameValue.trim()) return;

        try {
            await invoke('rename_project', {
                projectId: showRenameDialog,
                newTitle: renameValue.trim()
            });
            showToast('Project renamed successfully', 'success');
        } catch (error) {
            showToast(`Failed to rename project: ${error}`, 'error');
        } finally {
            setShowRenameDialog(null);
            setRenameValue('');
        }
    };

    const formatBytes = (bytes: number): string => {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
    };

    const handleExport = async (projectId: string) => {
        const project = projects.find(p => p.id === projectId);
        if (!project) return;

        try {
            setActiveMenu(null);

            // Open save dialog
            const savePath = await save({
                defaultPath: `${project.title.replace(/\s+/g, '_')}_export_${Date.now()}.tar.gz`,
                filters: [{ name: 'Project Archive', extensions: ['tar.gz', 'tgz'] }]
            });

            if (!savePath) return; // User cancelled

            const metadata = await invoke<ExportMetadata>('export_project', {
                projectId,
                exportPath: savePath
            });

            showToast(
                `✅ Exported "${metadata.project_title}": ${metadata.file_count} files, ${formatBytes(metadata.total_size)}`,
                'success'
            );
        } catch (error) {
            showToast(`❌ Failed to export project: ${error}`, 'error');
        }
    };

    const handleImport = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Project Archive', extensions: ['tar.gz', 'tgz'] }]
            });

            if (!selected) return; // User cancelled

            const result = await invoke<ImportResult>('import_project', {
                importPath: selected
            });

            showToast(
                `✅ Imported "${result.imported_title}": ${result.task_count} tasks, ${result.file_count} files`,
                'success'
            );

            // Refresh project list
            const activeProjects = await invoke<ProjectInfo[]>('get_active_projects');
            setProjects(activeProjects);
        } catch (error) {
            showToast(`❌ Failed to import project: ${error}`, 'error');
        }
    };

    const getAgentIcon = (type: string) => {
        switch (type) {
            case 'Admin': return '👑';
            case 'PM': return '📋';
            case 'Worker': return '👷';
            default: return '🤖';
        }
    };

    const getStatusColor = (state: string) => {
        switch (state) {
            case 'Idle': return 'bg-green-500';
            case 'Working': return 'bg-blue-500';
            case 'Planning': return 'bg-purple-500';
            case 'Reporting': return 'bg-yellow-500';
            case 'Error': return 'bg-red-500';
            default: return 'bg-gray-500';
        }
    };

    return (
        <div className="h-full flex flex-col text-gray-300 p-4 space-y-4">
            {/* Toast Notification */}
            {toast && (
                <div className={`fixed top-4 right-4 z-50 px-4 py-3 rounded-lg shadow-lg ${toast.type === 'success'
                    ? 'bg-green-600 text-white'
                    : 'bg-red-600 text-white'
                    }`}>
                    {toast.message}
                </div>
            )}

            {/* Rename Dialog */}
            {showRenameDialog && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                    <div className="bg-gray-800 rounded-lg p-6 shadow-xl max-w-md w-full mx-4">
                        <h3 className="text-lg font-semibold text-white mb-4">Rename Project</h3>
                        <input
                            type="text"
                            value={renameValue}
                            onChange={(e) => setRenameValue(e.target.value)}
                            className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded text-white focus:outline-none focus:border-blue-500"
                            placeholder="Enter new project title"
                            autoFocus
                            onKeyDown={(e) => {
                                if (e.key === 'Enter') confirmRename();
                                if (e.key === 'Escape') {
                                    setShowRenameDialog(null);
                                    setRenameValue('');
                                }
                            }}
                        />
                        <div className="flex gap-3 mt-4 justify-end">
                            <button
                                onClick={() => {
                                    setShowRenameDialog(null);
                                    setRenameValue('');
                                }}
                                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm text-white transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={confirmRename}
                                disabled={!renameValue.trim()}
                                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded text-sm text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                Rename
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Confirmation Dialog */}
            {showConfirmDialog && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                    <div className="bg-gray-800 rounded-lg p-6 shadow-xl max-w-md w-full mx-4">
                        <h3 className="text-lg font-semibold text-white mb-2">
                            {showConfirmDialog.action === 'stop' ? 'Stop Project?' : 'Delete Project?'}
                        </h3>
                        <p className="text-gray-400 mb-4">
                            {showConfirmDialog.action === 'stop'
                                ? 'This will cancel the project and clean up all associated agents. This action cannot be undone.'
                                : 'This will permanently delete the project and all its data. This action cannot be undone.'}
                        </p>
                        <div className="flex gap-3 justify-end">
                            <button
                                onClick={() => setShowConfirmDialog(null)}
                                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm text-white transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={confirmAction}
                                className="px-4 py-2 bg-red-600 hover:bg-red-500 rounded text-sm text-white transition-colors"
                            >
                                {showConfirmDialog.action === 'stop' ? 'Stop' : 'Delete'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Agents Section */}
            <div className="flex flex-col min-h-0 flex-1">
                <h2 className="text-lg font-semibold mb-2 text-white flex items-center gap-2">
                    <span>👥</span> Active Agents
                </h2>

                <div className="overflow-y-auto space-y-3 pr-1">
                    {agents.length === 0 ? (
                        <div className="text-sm text-gray-500 text-center py-4">
                            No active agents
                        </div>
                    ) : (
                        agents.map((agent) => (
                            <div
                                key={`${agent.id.type}-${agent.id.name}`}
                                className="bg-gray-700/50 rounded-lg p-3 border border-gray-600 hover:border-gray-500 transition-colors"
                            >
                                <div className="flex items-start justify-between mb-2">
                                    <div className="flex items-center gap-2">
                                        <span className="text-xl" title={agent.id.type}>
                                            {getAgentIcon(agent.id.type)}
                                        </span>
                                        <div>
                                            <div className="font-medium text-white text-sm">
                                                {agent.id.name}
                                            </div>
                                            <div className="text-xs text-gray-400">
                                                {agent.id.type}
                                            </div>
                                        </div>
                                    </div>
                                    {agent.status && (
                                        <div className={`w-2 h-2 rounded-full ${getStatusColor(agent.status.state)}`} title={agent.status.state} />
                                    )}
                                </div>

                                {agent.status && (
                                    <div className="mt-2 pt-2 border-t border-gray-600/50">
                                        <div className="flex justify-between items-center mb-1">
                                            <span className="text-xs font-medium text-gray-400">
                                                {agent.status.state}
                                            </span>
                                            <span className="text-[10px] text-gray-500">
                                                {new Date(agent.status.last_updated * 1000).toLocaleTimeString()}
                                            </span>
                                        </div>
                                        <div className="text-xs text-gray-300 italic truncate" title={agent.status.activity}>
                                            {agent.status.activity}
                                        </div>
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* Projects Section */}
            <div className="flex flex-col min-h-0 flex-1 border-t border-gray-700 pt-4">
                <div className="flex items-center justify-between mb-2">
                    <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                        <span>🚀</span> Active Projects
                    </h2>
                    <button
                        onClick={handleImport}
                        className="px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs text-white transition-colors flex items-center gap-1"
                        title="Import Project"
                    >
                        <span>📥</span> Import
                    </button>
                </div>

                <div className="overflow-y-auto space-y-3 pr-1">
                    {projects.length === 0 ? (
                        <div className="text-sm text-gray-500 text-center py-4">
                            No active projects
                        </div>
                    ) : (
                        projects.map(project => (
                            <div key={project.id} className="bg-gray-700/50 rounded-lg border border-gray-600 overflow-hidden">
                                <div className="flex items-center justify-between p-3 hover:bg-gray-600/50 transition-colors">
                                    <button
                                        onClick={() => toggleProject(project.id)}
                                        className="flex-1 text-left flex items-center justify-between pr-2"
                                    >
                                        <div className="font-medium text-white text-sm truncate pr-2" title={project.title}>
                                            {project.title}
                                        </div>
                                        <span className="text-xs text-gray-400">
                                            {expandedProjects.has(project.id) ? '▼' : '▶'}
                                        </span>
                                    </button>

                                    {/* Menu Button */}
                                    <div className="relative" ref={activeMenu === project.id ? menuRef : null}>
                                        <button
                                            onClick={(e) => toggleMenu(project.id, e)}
                                            className="p-1.5 hover:bg-gray-600 rounded transition-colors"
                                            title="Project actions"
                                        >
                                            <span className="text-gray-400 text-sm">⋮</span>
                                        </button>

                                        {/* Dropdown Menu */}
                                        {activeMenu === project.id && (
                                            <div className="absolute right-0 mt-1 w-40 bg-gray-800 border border-gray-600 rounded-lg shadow-xl z-10">
                                                {project.status === 'Paused' ? (
                                                    <button
                                                        onClick={() => handleResume(project.id)}
                                                        className="w-full px-4 py-2 text-left text-sm text-white hover:bg-gray-700 flex items-center gap-2"
                                                    >
                                                        <span>▶️</span> Resume
                                                    </button>
                                                ) : (
                                                    <button
                                                        onClick={() => handlePause(project.id)}
                                                        className="w-full px-4 py-2 text-left text-sm text-white hover:bg-gray-700 flex items-center gap-2"
                                                    >
                                                        <span>⏸️</span> Pause
                                                    </button>
                                                )}
                                                <button
                                                    onClick={() => handleStop(project.id)}
                                                    className="w-full px-4 py-2 text-left text-sm text-white hover:bg-gray-700 flex items-center gap-2"
                                                >
                                                    <span>⏹️</span> Stop
                                                </button>
                                                <button
                                                    onClick={() => handleRename(project.id)}
                                                    className="w-full px-4 py-2 text-left text-sm text-white hover:bg-gray-700 flex items-center gap-2"
                                                >
                                                    <span>✏️</span> Rename
                                                </button>
                                                <button
                                                    onClick={() => handleExport(project.id)}
                                                    className="w-full px-4 py-2 text-left text-sm text-white hover:bg-gray-700 flex items-center gap-2"
                                                >
                                                    <span>📦</span> Export
                                                </button>
                                                <button
                                                    onClick={() => handleDelete(project.id)}
                                                    className="w-full px-4 py-2 text-left text-sm text-red-400 hover:bg-gray-700 flex items-center gap-2 border-t border-gray-600"
                                                >
                                                    <span>🗑️</span> Delete
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </div>

                                {expandedProjects.has(project.id) && (
                                    <div className="p-3 pt-0 border-t border-gray-600/30 bg-gray-800/30">
                                        <div className="text-xs text-gray-400 mb-2 mt-2 flex justify-between">
                                            <span>Unfinished Tasks</span>
                                            <span className="bg-gray-700 px-1 rounded text-[10px]">{project.status}</span>
                                        </div>
                                        {project.tasks.length === 0 ? (
                                            <div className="text-xs text-gray-500 italic">All tasks complete</div>
                                        ) : (
                                            <ul className="space-y-2">
                                                {project.tasks.map(task => (
                                                    <li key={task.id} className="text-xs flex flex-col gap-1">
                                                        <div className="flex items-start gap-2">
                                                            <span className="text-yellow-500 mt-0.5">•</span>
                                                            <span className="text-gray-300">{task.title}</span>
                                                        </div>
                                                        <span className="text-[10px] text-gray-500 ml-4">
                                                            {task.status}
                                                        </span>
                                                    </li>
                                                ))}
                                            </ul>
                                        )}
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </div>
        </div>
    );
}
