import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { GuideApp } from './GuideApp'
import { zh } from './content/zh'
import './styles/theme.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <GuideApp c={zh} />
  </StrictMode>,
)
