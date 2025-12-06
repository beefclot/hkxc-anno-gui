import { useState, useEffect, useRef } from 'react'
import { open } from '@tauri-apps/api/dialog'
import { invoke } from '@tauri-apps/api/tauri'
import { listen } from '@tauri-apps/api/event'
import Editor from '@monaco-editor/react'

interface Tab {
  hkxPath: string     // Original HKX file path
  annoPath: string    // Annotation .txt file path
  displayName: string
  content: string
  modified: boolean
}

interface StatusMessage {
  type: 'idle' | 'loading' | 'success' | 'error'
  message: string
}

function App() {
  const [tabs, setTabs] = useState<Tab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)
  const [format, setFormat] = useState<'amd64' | 'win32'>('amd64')
  const [status, setStatus] = useState<StatusMessage>({ type: 'idle', message: 'Ready' })
  const [isDragging, setIsDragging] = useState(false)
  const tabsRef = useRef<Tab[]>([])

  // Keep ref in sync with state
  useEffect(() => {
    tabsRef.current = tabs
  }, [tabs])

  const activeTab = tabs.find(t => t.annoPath === activeTabId)

  const showStatus = (type: StatusMessage['type'], message: string, duration = 3000) => {
    setStatus({ type, message })
    if (type !== 'loading') {
      setTimeout(() => setStatus({ type: 'idle', message: 'Ready' }), duration)
    }
  }

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{ name: 'HKX Files', extensions: ['hkx'] }]
      })
      
      if (selected && Array.isArray(selected)) {
        handleDump(selected)
      }
    } catch (error) {
      showStatus('error', `Error selecting files: ${error}`)
    }
  }

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true
      })
      
      if (selected && typeof selected === 'string') {
        handleDump([selected])
      }
    } catch (error) {
      showStatus('error', `Error selecting folder: ${error}`)
    }
  }

  const handleDump = async (paths: string[]) => {
    showStatus('loading', 'Dumping annotations...')
    try {
      const results = await invoke<Array<{
        hkx_path: string
        anno_path: string
        display_name: string
        content: string
      }>>('dump_annotations', {
        input: paths
      })

      const newTabs: Tab[] = results.map(r => ({
        hkxPath: r.hkx_path,
        annoPath: r.anno_path,
        displayName: r.display_name,
        content: r.content,
        modified: false
      }))

      setTabs(prev => [...prev, ...newTabs])

      if (newTabs.length > 0) {
        setActiveTabId(newTabs[0].annoPath)
      }

      showStatus('success', `Dumped ${results.length} file(s)`)
    } catch (error) {
      showStatus('error', `Dump failed: ${error}`)
    }
  }

  const handleEditorChange = (value: string | undefined) => {
    if (!activeTabId || value === undefined) return
    
    setTabs(prev => prev.map(tab => 
      tab.annoPath === activeTabId 
        ? { ...tab, content: value, modified: true }
        : tab
    ))
  }

  const handleSave = async () => {
    if (!activeTab) return

    showStatus('loading', 'Updating annotations...')
    try {
      await invoke('update_annotations', {
        files: [{
          hkx_path: activeTab.hkxPath,
          anno_path: activeTab.annoPath,
          display_name: activeTab.displayName,
          content: activeTab.content
        }],
        format
      })

      setTabs(prev => prev.map(tab =>
        tab.annoPath === activeTabId ? { ...tab, modified: false } : tab
      ))

      showStatus('success', 'Saved successfully')
    } catch (error) {
      showStatus('error', `Save failed: ${error}`)
    }
  }

  const handleSaveAll = async () => {
    const modifiedTabs = tabs.filter(t => t.modified)
    if (modifiedTabs.length === 0) return

    showStatus('loading', `Saving ${modifiedTabs.length} file(s)...`)
    try {
      await invoke('update_annotations', {
        files: modifiedTabs.map(t => ({
          hkx_path: t.hkxPath,
          anno_path: t.annoPath,
          display_name: t.displayName,
          content: t.content
        })),
        format
      })

      setTabs(prev => prev.map(tab => ({ ...tab, modified: false })))
      showStatus('success', `Saved ${modifiedTabs.length} file(s)`)
    } catch (error) {
      showStatus('error', `Bulk save failed: ${error}`)
    }
  }

  const handleCloseTab = async (annoPath: string) => {
    // Delete the annotation file
    try {
      await invoke('cleanup_annotation', { annoPath })
    } catch (error) {
      console.warn('Failed to cleanup annotation file:', error)
    }
    
    setTabs(prev => {
      const newTabs = prev.filter(t => t.annoPath !== annoPath)
      if (activeTabId === annoPath && newTabs.length > 0) {
        setActiveTabId(newTabs[0].annoPath)
      } else if (newTabs.length === 0) {
        setActiveTabId(null)
      }
      return newTabs
    })
  }

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 's') {
        e.preventDefault()
        if (e.shiftKey) {
          handleSaveAll()
        } else {
          handleSave()
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [activeTab, tabs, format])

  // Drag and drop listener
  useEffect(() => {
    const unlisten = listen<string[]>('tauri://file-drop', (event) => {
      setIsDragging(false)
      handleDump(event.payload)
    })

    const unlistenHover = listen('tauri://file-drop-hover', () => {
      setIsDragging(true)
    })

    const unlistenCancelled = listen('tauri://file-drop-cancelled', () => {
      setIsDragging(false)
    })

    return () => {
      unlisten.then(fn => fn())
      unlistenHover.then(fn => fn())
      unlistenCancelled.then(fn => fn())
    }
  }, [])

  // Cleanup on app close - using ref to avoid dependency issues
  useEffect(() => {
    const handleBeforeUnload = () => {
      // Use ref to get current tabs without adding to dependencies
      const currentTabs = tabsRef.current
      if (currentTabs.length > 0) {
        const annoPaths = currentTabs.map(t => t.annoPath)
        // Note: beforeunload may not wait for async, so this is best-effort
        invoke('cleanup_all_annotations', { annoPaths }).catch(err => {
          console.warn('Failed to cleanup annotations on exit:', err)
        })
      }
    }

    window.addEventListener('beforeunload', handleBeforeUnload)
    
    // Cleanup when component unmounts (app closes)
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload)
      
      // Cleanup all annotation files on unmount
      const currentTabs = tabsRef.current
      if (currentTabs.length > 0) {
        const annoPaths = currentTabs.map(t => t.annoPath)
        invoke('cleanup_all_annotations', { annoPaths }).catch(err => {
          console.warn('Failed to cleanup annotations on unmount:', err)
        })
      }
    }
  }, []) // Empty dependency array - only run on mount/unmount

  return (
    <div className={`app ${isDragging ? 'dragging' : ''}`}>
      <div className="header">
        <h1>HKXC Annotation Editor (Tauri)</h1>
        <button className="btn" onClick={handleSelectFile}>
          Open File(s)
        </button>
        <button className="btn" onClick={handleSelectFolder}>
          Open Folder
        </button>
        <select 
          className="format-select" 
          value={format} 
          onChange={e => setFormat(e.target.value as 'amd64' | 'win32')}
        >
          <option value="amd64">64-bit (SE/AE)</option>
          <option value="win32">32-bit (LE)</option>
        </select>
        <button 
          className="btn btn-success" 
          onClick={handleSave}
          disabled={!activeTab || !activeTab.modified}
        >
          Save (Ctrl+S)
        </button>
        <button 
          className="btn btn-success" 
          onClick={handleSaveAll}
          disabled={tabs.filter(t => t.modified).length === 0}
        >
          Save All (Ctrl+Shift+S)
        </button>
      </div>

      <div className="tabs">
        {tabs.map(tab => (
          <button
            key={tab.annoPath}
            className={`tab ${tab.annoPath === activeTabId ? 'active' : ''}`}
            onClick={() => setActiveTabId(tab.annoPath)}
          >
            {tab.displayName}{tab.modified && ' •'}
            <span className="tab-close" onClick={(e) => {
              e.stopPropagation()
              handleCloseTab(tab.annoPath)
            }}>
              ×
            </span>
          </button>
        ))}
      </div>

      <div className="editor-container">
        {isDragging && (
          <div className="drop-overlay">
            <div className="drop-message">
              <div className="drop-icon">📁</div>
              <h2>Drop HKX files or folders here</h2>
              <p>Release to dump annotations</p>
            </div>
          </div>
        )}
        {activeTab ? (
          <Editor
            height="100%"
            defaultLanguage="plaintext"
            theme="vs-dark"
            value={activeTab.content}
            onChange={handleEditorChange}
            options={{
              minimap: { enabled: false },
              fontSize: 14,
              lineNumbers: 'on',
              rulers: [80],
              wordWrap: 'off',
            }}
          />
        ) : (
          <div className="welcome">
            <h2>Welcome to HKXC Annotation Editor</h2>
            <p>Open HKX file(s) or folder to start editing annotations</p>
            <p><strong>Or drag & drop files/folders here</strong></p>
            <div>
              <span className="kbd">Ctrl+S</span> Save current file • 
              <span className="kbd">Ctrl+Shift+S</span> Save all modified files
            </div>
          </div>
        )}
      </div>

      <div className={`status-bar ${status.type}`}>
        {status.type === 'loading' && <div className="spinner"></div>}
        {status.message}
      </div>
    </div>
  )
}

export default App

