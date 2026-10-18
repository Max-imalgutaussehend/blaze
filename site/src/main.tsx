import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@fontsource-variable/inter'
import '@fontsource-variable/geist-mono'
import './index.css'
import App from './App'
import { ToolPage } from './pages/ToolPage'

const root = document.getElementById('root')!
const page = root.dataset.page

createRoot(root).render(<StrictMode>{page ? <ToolPage id={page} /> : <App />}</StrictMode>)
