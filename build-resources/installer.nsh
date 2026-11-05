; Extra pages and text for the FreeSheet Windows installer (NSIS, included by electron-builder).

!macro customWelcomePage
  !define MUI_WELCOMEPAGE_TITLE "Welcome to FreeSheet"
  !define MUI_WELCOMEPAGE_TEXT "FreeSheet gives you study notes, practice questions and full exam papers for Grades 8 to 12, following the CAPS curriculum.$\r$\n$\r$\n\
  • Notes are shown step by step, and can be read aloud$\r$\n\
  • Questions are marked instantly, with explanations$\r$\n\
  • Everything works offline, with no account needed$\r$\n$\r$\n\
  Your progress is saved on this computer only.$\r$\n$\r$\n\
  Click Next to continue."
  !insertmacro MUI_PAGE_WELCOME
!macroend
