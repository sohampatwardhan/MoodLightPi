const WIDTH = 8, HEIGHT = 4;
const canvas = document.getElementById('preview');
const ctx = canvas.getContext('2d');
const cell = canvas.width / WIDTH;

function draw(pixels) {
  for (let y = 0; y < HEIGHT; y++) {
    for (let x = 0; x < WIDTH; x++) {
      const [r, g, b] = pixels[y * WIDTH + x];
      ctx.fillStyle = `rgb(${r},${g},${b})`;
      ctx.fillRect(x * cell, y * cell, cell, cell);
    }
  }
}

function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (e) => { draw(JSON.parse(e.data).pixels); };
  ws.onclose = () => setTimeout(connect, 1000);
}
connect();

async function post(path, body) {
  await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
}

document.getElementById('power').onclick = async () => {
  const s = await (await fetch('/api/state')).json();
  await post('/api/power', { on: !s.state.power });
};
document.getElementById('color').oninput = (e) => {
  const v = e.target.value;
  post('/api/color', {
    r: parseInt(v.slice(1, 3), 16),
    g: parseInt(v.slice(3, 5), 16),
    b: parseInt(v.slice(5, 7), 16),
  });
};
document.getElementById('brightness').oninput = (e) =>
  post('/api/brightness', { value: parseInt(e.target.value, 10) });
document.getElementById('speed').oninput = (e) => {
  const name = document.getElementById('effect').value;
  post('/api/effect', { name, speed: parseInt(e.target.value, 10) });
};
document.getElementById('effect').onchange = (e) =>
  post('/api/effect', { name: e.target.value });

(async () => {
  const { effects } = await (await fetch('/api/effects')).json();
  const sel = document.getElementById('effect');
  for (const name of effects) {
    const opt = document.createElement('option');
    opt.value = name; opt.textContent = name; sel.appendChild(opt);
  }
})();
