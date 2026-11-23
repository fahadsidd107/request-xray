import express from 'express';
import http from 'node:http';
import { addEvent, getActiveSpan, getXRay, withSpan } from '@request-xray/node';

const downstream = process.env.DOWNSTREAM_URL ?? 'http://127.0.0.1:3901';
const app = express();
app.get('/orders/:id', async (req, res, next) => {
  try {
    const result = await withSpan('orders.lookup', async span => {
      const target = new URL(`/items/${encodeURIComponent(req.params.id)}`, downstream);
      target.searchParams.set('token', req.query.token ?? 'example');
      const item = await fetch(target, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ example: req.query.token ?? 'example' }),
      }).then(response => response.json());
      await new Promise((resolve, reject) => {
        http.get(target, response => {
          response.resume();
          response.once('end', resolve);
          response.once('error', reject);
        }).once('error', reject);
      });
      addEvent('orders.lookup.complete', { 'item.found': Boolean(item.id) });
      return { ...item, activeSpanPreserved: getActiveSpan() === span };
    });
    res.json(result);
  } catch (error) {
    next(error);
  }
});

const server = app.listen(Number(process.env.PORT ?? 3900), '127.0.0.1', () => {
  console.log(`Listening on http://127.0.0.1:${server.address().port}`);
});
process.once('SIGTERM', () => server.close(async () => {
  await getXRay()?.shutdown();
}));
