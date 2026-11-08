# FreeSheet website

`server.js` serves FreeSheet as a website for phones and computers. It needs Node.js 20 or newer and no
other packages. It uses `src/`, `content/` and `audio/` from this repository, so clone the whole thing.

## Start it

```sh
git clone --branch rust-native-ui https://github.com/Varmite/freesheet.git
cd freesheet
cp server/.env.example server/.env   # then fill it in (next sections)
node --env-file=server/.env server/server.js
```

It listens on port 8080. On your own network, open `http://<laptop-ip>:8080`. Without a `.env` file, run
`node server/server.js`: everything works except signing in.

## Put it on the internet

Google sign-in needs an `https://` address with a real domain name, so a bare IP address won't do. The
laptop also has to stay on and awake. A tunnel avoids opening ports on your router:

**Tailscale Funnel** (free, no domain needed; gives you `https://<laptop>.<tailnet>.ts.net`):

```sh
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up
sudo tailscale funnel --bg 8080
```

**Cloudflare Tunnel** works too if you have a domain on Cloudflare.

Put the address in `PUBLIC_URL` and keep `TRUST_PROXY=1`.

## Turn on Google sign-in

1. Go to <https://console.cloud.google.com/> and create a project.
2. Under **Google Auth Platform**, set up the app (External) and add your email address.
3. Under **Clients**, create a client of type **Web application**. Add this **Authorised redirect URI**,
   with your own address: `https://<your-address>/auth/google/callback`
4. Copy the client ID and secret into `server/.env` and restart the server.
5. Under **Audience**, press **Publish app**. Until then only test users you list can sign in.

## Import your progress into your Google account

1. On the computer with your progress, open FreeSheet and use **Settings > Export progress**.
2. On the website, go to **Settings > Sign in with Google**.
3. Then **Settings > Import progress**, and choose the exported file. It replaces what the account has.

## How progress is tracked

Web pages cannot read a device's or operating system's real ID, so each browser gets a random UUID in a
cookie on its first visit, and progress is saved under it. Clearing the browser's data loses that link.

Signing in moves that browser's progress to the account if the account has none yet. If the account already
has progress, the account's is used and the browser's is left alone. Signing out goes back to the browser's.

Each learner is one file in `server/data/owners/`, and the last version is kept as `.bak`. Back up that
folder. Sign-in stores only Google's account ID (hashed) and the email address.

## Keep it running

`/etc/systemd/system/freesheet.service`, with your own user name and path:

```ini
[Unit]
Description=FreeSheet website
After=network-online.target

[Service]
User=you
WorkingDirectory=/home/you/freesheet
ExecStart=/usr/bin/node --env-file=server/.env server/server.js
Restart=always

[Install]
WantedBy=multi-user.target
```

Then `sudo systemctl enable --now freesheet`. To update: `git pull` and `sudo systemctl restart freesheet`.
