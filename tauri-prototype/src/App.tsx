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

interface SearchMatch {
  line: number
  column: number
  text: string
}

interface SearchResult {
  tabId: string
  displayName: string
  matches: SearchMatch[]
}

function App() {
  const [tabs, setTabs] = useState<Tab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)
  const [format, setFormat] = useState<'amd64' | 'win32'>('amd64')
  const [status, setStatus] = useState<StatusMessage>({ type: 'idle', message: 'Ready' })
  const [isDragging, setIsDragging] = useState(false)
  const tabsRef = useRef<Tab[]>([])
  
  // Global search state
  const [showSearch, setShowSearch] = useState(false)
  const [searchMode, setSearchMode] = useState<'find' | 'replace'>('find')
  const [searchQuery, setSearchQuery] = useState('')
  const [replaceQuery, setReplaceQuery] = useState('')
  const [searchResults, setSearchResults] = useState<SearchResult[]>([])
  const searchInputRef = useRef<HTMLInputElement>(null)
  
  // Hotkeys modal state
  const [showHotkeys, setShowHotkeys] = useState(false)

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

  const handleCloseTab = (annoPath: string) => {
    // No cleanup needed - files are in memory only
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

  // Global search functions
  const performSearch = (query: string) => {
    if (!query.trim()) {
      setSearchResults([])
      return
    }

    const results: SearchResult[] = []
    const queryLower = query.toLowerCase()

    tabs.forEach(tab => {
      const lines = tab.content.split('\n')
      const matches: SearchMatch[] = []

      lines.forEach((line, lineIndex) => {
        const lineLower = line.toLowerCase()
        let index = 0
        while ((index = lineLower.indexOf(queryLower, index)) !== -1) {
          const contextStart = Math.max(0, index - 30)
          const contextEnd = Math.min(line.length, index + query.length + 30)
          matches.push({
            line: lineIndex + 1,
            column: index + 1,
            text: (contextStart > 0 ? '...' : '') + 
                  line.substring(contextStart, contextEnd) + 
                  (contextEnd < line.length ? '...' : '')
          })
          index += query.length
        }
      })

      if (matches.length > 0) {
        results.push({
          tabId: tab.annoPath,
          displayName: tab.displayName,
          matches
        })
      }
    })

    setSearchResults(results)
  }

  const handleReplaceAll = () => {
    if (!searchQuery.trim()) return

    let totalReplacements = 0
    const escapedQuery = searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const regex = new RegExp(escapedQuery, 'gi')

    setTabs(prev => prev.map(tab => {
      const matchCount = (tab.content.match(regex) || []).length
      if (matchCount > 0) {
        totalReplacements += matchCount
        return {
          ...tab,
          content: tab.content.replace(regex, replaceQuery),
          modified: true
        }
      }
      return tab
    }))

    setSearchResults([])
    setShowSearch(false)
    showStatus('success', `Replaced ${totalReplacements} occurrence(s) across all files`)
  }

  const openSearch = (mode: 'find' | 'replace') => {
    setSearchMode(mode)
    setShowSearch(true)
    setSearchQuery('')
    setReplaceQuery('')
    setSearchResults([])
    setTimeout(() => searchInputRef.current?.focus(), 50)
  }

  const closeSearch = () => {
    setShowSearch(false)
    setSearchResults([])
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
      } else if (e.ctrlKey && (e.key === 'w' || e.key === 'W')) {
        e.preventDefault()
        if (e.shiftKey) {
          // Ctrl+Shift+W: Close all tabs
          setTabs([])
          setActiveTabId(null)
        } else {
          // Ctrl+W: Close current tab
          if (activeTabId) {
            handleCloseTab(activeTabId)
          }
        }
      } else if (e.ctrlKey && e.shiftKey && (e.key === 'f' || e.key === 'F')) {
        e.preventDefault()
        openSearch('find')
      } else if (e.ctrlKey && e.shiftKey && (e.key === 'h' || e.key === 'H')) {
        e.preventDefault()
        openSearch('replace')
      } else if (e.key === 'Escape') {
        if (showHotkeys) {
          e.preventDefault()
          setShowHotkeys(false)
        } else if (showSearch) {
          e.preventDefault()
          closeSearch()
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [activeTab, tabs, format, activeTabId, showSearch, showHotkeys])

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

  // No cleanup needed on app close - files are kept in memory only

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
        <button 
          className="btn btn-help" 
          onClick={() => setShowHotkeys(true)}
          title="Keyboard Shortcuts"
        >
          ?
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

      {showSearch && (
        <div className="search-panel">
          <div className="search-header">
            <span className="search-title">
              {searchMode === 'find' ? 'Find in All Files' : 'Find & Replace in All Files'}
            </span>
            <button className="search-close" onClick={closeSearch}>×</button>
          </div>
          <div className="search-inputs">
            <input
              ref={searchInputRef}
              type="text"
              className="search-input"
              placeholder="Search..."
              value={searchQuery}
              onChange={(e) => {
                setSearchQuery(e.target.value)
                performSearch(e.target.value)
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && searchMode === 'replace') {
                  handleReplaceAll()
                }
              }}
            />
            {searchMode === 'replace' && (
              <input
                type="text"
                className="search-input"
                placeholder="Replace with..."
                value={replaceQuery}
                onChange={(e) => setReplaceQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    handleReplaceAll()
                  }
                }}
              />
            )}
          </div>
          {searchMode === 'replace' && searchQuery && (
            <button 
              className="btn btn-replace"
              onClick={handleReplaceAll}
              disabled={!searchQuery.trim()}
            >
              Replace All ({searchResults.reduce((sum, r) => sum + r.matches.length, 0)})
            </button>
          )}
          {searchResults.length > 0 && (
            <div className="search-results">
              <div className="search-results-summary">
                {searchResults.reduce((sum, r) => sum + r.matches.length, 0)} results in {searchResults.length} file(s)
              </div>
              {searchResults.map(result => (
                <div key={result.tabId} className="search-result-file">
                  <div 
                    className="search-result-filename"
                    onClick={() => {
                      setActiveTabId(result.tabId)
                    }}
                  >
                    {result.displayName} ({result.matches.length})
                  </div>
                  <div className="search-result-matches">
                    {result.matches.slice(0, 5).map((match, idx) => (
                      <div 
                        key={idx} 
                        className="search-match"
                        onClick={() => setActiveTabId(result.tabId)}
                      >
                        <span className="match-line">Line {match.line}:</span> {match.text}
                      </div>
                    ))}
                    {result.matches.length > 5 && (
                      <div className="search-match-more">
                        ...and {result.matches.length - 5} more
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
          {searchQuery && searchResults.length === 0 && (
            <div className="search-no-results">No results found</div>
          )}
        </div>
      )}

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
              wordWrap: 'on',
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

      {showHotkeys && (
        <div className="modal-overlay" onClick={() => setShowHotkeys(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>Keyboard Shortcuts</h2>
              <button className="modal-close" onClick={() => setShowHotkeys(false)}>×</button>
            </div>
            <div className="modal-content">
              <table className="hotkeys-table">
                <tbody>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>S</kbd></td><td>Save current file</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd></td><td>Save all modified files</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>W</kbd></td><td>Close current tab</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>W</kbd></td><td>Close all tabs</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd></td><td>Find in all files</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>H</kbd></td><td>Find & replace in all files</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>F</kbd></td><td>Find in current file</td></tr>
                  <tr><td><kbd>Ctrl</kbd>+<kbd>H</kbd></td><td>Find & replace in current file</td></tr>
                  <tr><td><kbd>Escape</kbd></td><td>Close search panel / modal</td></tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      <div className={`status-bar ${status.type}`}>
        {status.type === 'loading' && <div className="spinner"></div>}
        {status.message}
      </div>
    </div>
  )
}

export default App

