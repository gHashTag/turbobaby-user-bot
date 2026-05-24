const express = require('express');
const multer = require('multer');
const cors = require('cors');
const path = require('path');
const fs = require('fs');

const app = express();
app.use(cors());
app.use(express.json());

const uploadDir = path.join(__dirname, 'uploads');
if (!fs.existsSync(uploadDir)) fs.mkdirSync(uploadDir, { recursive: true });

const storage = multer.diskStorage({
  destination: uploadDir,
  filename: (req, file, cb) => {
    const unique = Date.now() + '-' + Math.round(Math.random() * 1e9);
    cb(null, unique + '-' + file.originalname);
  }
});
const upload = multer({ storage, limits: { fileSize: 10 * 1024 * 1024 } });

// Auth check
app.get('/api/admin/check', (req, res) => {
  console.log('[mock] /api/admin/check headers:', JSON.stringify(req.headers));
  res.json({ is_admin: true });
});

// Upload
app.post('/api/upload', upload.single('file'), (req, res) => {
  console.log('[mock] /api/upload headers:', JSON.stringify(req.headers));
  
  if (!req.file) {
    return res.status(400).json({ error: 'No file' });
  }
  const url = `/uploads/${req.file.filename}`;
  res.json({ url });
});

// Serve uploaded files
app.use('/uploads', express.static(uploadDir));

// Ping
app.get('/api/ping', (req, res) => res.json({ status: 'ok' }));

const PORT = 3001;
app.listen(PORT, () => {
  console.log(`Mock API running on http://localhost:${PORT}`);
});
