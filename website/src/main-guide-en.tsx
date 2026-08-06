import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { GuideApp } from './GuideApp'
import { en } from './content/en'
import './styles/theme.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <GuideApp c={en} />
  </StrictMode>,
)
