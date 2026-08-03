import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { zh } from './content/zh'
import './styles/theme.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App c={zh} />
  </StrictMode>,
)
