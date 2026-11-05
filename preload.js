'use strict';
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
  getCatalog: () => ipcRenderer.invoke('catalog:get'),
  getTopic: (grade, subjectId, topicId) => ipcRenderer.invoke('topic:get', grade, subjectId, topicId),
  getExam: (grade, subjectId, examId) => ipcRenderer.invoke('exam:get', grade, subjectId, examId),
  loadProgress: () => ipcRenderer.invoke('progress:load'),
  saveProgress: (data) => ipcRenderer.invoke('progress:save', data),
  saveProgressSync: (data) => ipcRenderer.sendSync('progress:save-sync', data),
  progressLocation: () => ipcRenderer.invoke('progress:location'),
  exportProgress: (data) => ipcRenderer.invoke('progress:export', data),
  importProgress: () => ipcRenderer.invoke('progress:import'),
  openDataFolder: () => ipcRenderer.invoke('app:open-data-folder'),
  version: () => ipcRenderer.invoke('app:version'),
  listAudio: (grade, subjectId, topicId) => ipcRenderer.invoke('audio:list', grade, subjectId, topicId),
  getAudio: (grade, subjectId, topicId, id) => ipcRenderer.invoke('audio:get', grade, subjectId, topicId, id),
});
