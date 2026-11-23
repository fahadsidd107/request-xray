const express = require('express');
const { getXRay } = require('@request-xray/node');

const app = express();
function respond(req, res) {
  res.json({ id: req.params.id, traceparent: req.get('traceparent') });
}
app.get('/items/:id', respond);
app.post('/items/:id', express.json(), respond);

const server = app.listen(Number(process.env.PORT ?? 3901), '127.0.0.1', () => {
  console.log(`Listening on http://127.0.0.1:${server.address().port}`);
});
process.once('SIGTERM', () => server.close(async () => {
  await getXRay()?.shutdown();
}));
