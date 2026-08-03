import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { en } from './content/en'
import './styles/theme.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App c={en} />
  </StrictMode>,
)
