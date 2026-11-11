# FreeSheet web edition: setting it up on your laptop

This puts FreeSheet on your old Linux Mint laptop so you (and your sister) can use it from
your phones' browsers, anywhere, signed in with Google. Your progress is saved on the laptop,
separately for each Google account.

You will need about an hour the first time. Copy each command exactly: click in the
**Terminal** (Menu > Terminal), paste with **Ctrl+Shift+V**, and press **Enter**.

What you end up with:

```
phone (anywhere) ──https──> https://<name>.duckdns.org:5000 ──router port 5000──> laptop (freesheet-web)
```

---

## 0. Download and install

The FreeSheet repository is private, so first let the laptop sign in to GitHub:

```sh
sudo apt update && sudo apt install -y git gh
gh auth login          # choose GitHub.com, HTTPS, "Login with a web browser"
gh auth setup-git      # lets git (and the automatic updater) use that sign-in
```

Then download FreeSheet and install the web edition:

```sh
cd ~
git clone --branch rust-native-ui https://github.com/Varmite/freesheet.git
cd freesheet/web
./install.sh
```

`install.sh`:

- checks for Node.js 18 or newer, and offers to install Node.js 22 if it's missing;
- installs the two packages FreeSheet needs;
- asks for your **Google client ID** (from step 1; press Enter to skip it for now) and the
  **email addresses allowed to sign in** (yours and your sister's, separated by commas);
- creates the `freesheet-web` service, which runs as you, restarts if it crashes, and
  starts when the laptop boots;
- prints the address.

You can run `./install.sh` again at any time to change those answers.

Check that it works: on the laptop, open <http://localhost:5000>. You should see the
FreeSheet sign-in page. (Signing in only works after steps 1 to 3.)

> **Port 5000 is busy?** `install.sh` stops and tells you if another program is using
> port 5000. To find it:
>
> ```sh
> sudo ss -ltnp 'sport = :5000'
> ```
>
> The `users:(("name",pid=1234,...))` part names the program. If it's an old service:
>
> ```sh
> systemctl list-units --type=service --state=running   # find its name
> sudo systemctl disable --now <that-service-name>
> ```
>
> If it was an old Docker container: `docker ps`, then `docker stop <name>` and
> `docker update --restart=no <name>`. For anything else, `sudo kill <pid>`.

---

## 1. Google sign-in (free)

1. Go to <https://console.cloud.google.com/> and sign in with your Google account.
2. At the top, click the project list, then **New project**. Name it `FreeSheet` and click
   **Create**. Make sure it is selected.
3. In the search bar type **Google Auth Platform** and open it. Click **Get started**:
   - App name: `FreeSheet`; User support email: your email. **Next**.
   - Audience: **External**. **Next**.
   - Contact information: your email. **Next**, tick the agreement, **Create**.
4. Open **Audience** (left menu). Under **Test users** click **Add users** and add your Gmail
   address and your sister's. Leave the app in **Testing**: test users can sign in without
   Google having to review the app.
5. Open **Clients** > **Create client**:
   - Application type: **Web application**; Name: `FreeSheet web`.
   - **Authorised JavaScript origins** > **Add URI**: `https://<name>.duckdns.org:5000`
     (the name you choose in step 2; you can come back and add it later).
   - Leave **Authorised redirect URIs** empty.
   - Click **Create** and copy the **Client ID** (it ends in `.apps.googleusercontent.com`).
6. Put it on the laptop:

   ```sh
   cd ~/freesheet/web
   ./install.sh          # paste the client ID when asked; keep the other answers
   ```

Only email addresses in `ALLOWED_EMAILS` (in `web/.env`) can sign in, even if Google lets
someone else through.

---

## 2. A free web address: DuckDNS

Your home's internet address (IP) can change. DuckDNS gives you a free name that follows it.

1. Go to <https://www.duckdns.org> and sign in (with Google is easiest).
2. Type a subdomain, e.g. `reevesfamily`, and click **add domain**. Your address is now
   `reevesfamily.duckdns.org`.
3. Copy the **token** shown at the top of the page. Keep it secret: it controls your name.

Step 3's script sets up the updater that keeps the name pointing at your home.

---

## 3. HTTPS: a free certificate (Let's Encrypt)

Google sign-in refuses plain `http://` addresses, so FreeSheet needs a certificate. This uses
the **DNS challenge** through DuckDNS, so you **only forward port 5000**; ports 80 and 443
are not needed.

```sh
cd ~/freesheet/web
./setup-https.sh
```

It asks for your DuckDNS subdomain, token and email, then:

1. adds a cron job (`~/duckdns/duck.sh`, every 5 minutes) that keeps the name pointing at
   your home's IP;
2. installs [acme.sh](https://github.com/acmesh-official/acme.sh) and gets a Let's Encrypt
   certificate for `<name>.duckdns.org`. It waits about 2 minutes for DuckDNS, so be patient;
3. saves it in `web/certs/`, sets `HTTPS_CERT` and `HTTPS_KEY` in `web/.env`, and restarts
   FreeSheet.

**Renewal is automatic.** acme.sh checks every day and renews about every 60 days. After
renewing, it sends FreeSheet a signal (`kill -HUP`) and FreeSheet loads the new certificate
without restarting. (FreeSheet also checks the file twice a day, just in case.) To renew by hand:

```sh
~/.acme.sh/acme.sh --renew -d <name>.duckdns.org --force
```

Now open `https://<name>.duckdns.org:5000` on your phone **using mobile data** (Wi-Fi off),
sign in with Google, and you're in. On home Wi-Fi, see step 4 if it doesn't open.

Check the router: it should forward **external port 5000** (TCP) to the laptop's IP, port
5000. To see the laptop's IP: `hostname -I`. Give the laptop a fixed IP in the router
("DHCP reservation") so the forwarding keeps working.

---

## 4. "It works on mobile data but not on home Wi-Fi" (hairpin NAT)

Some routers can't send a request from inside your home out to your own public address and
back in again. This is called **hairpin NAT** or **NAT loopback**. Fixes, easiest first:

1. **Router setting:** look for **NAT loopback**, **hairpin NAT** or **NAT reflection** in
   the router's settings and turn it on.
2. **Local DNS override:** if your router (or Pi-hole) can set a local DNS name, point
   `<name>.duckdns.org` at the laptop's home IP (from `hostname -I`). The certificate still
   matches because the name is the same.
3. **Just for one device:** on a laptop, add a line to `/etc/hosts` such as
   `192.168.1.50  reevesfamily.duckdns.org`. (Phones can't do this without an app.)

---

## 5. On your phone

**Add to home screen**, so it opens like an app:

- **iPhone (Safari):** open the address, tap **Share** (the square with an arrow), then
  **Add to Home Screen**.
- **Android (Chrome):** open the address, tap **⋮**, then **Add to Home screen** or
  **Install app**.

**Read-aloud** uses Ogg Opus audio. It works in Chrome, Firefox and Edge, and in Safari on
iOS 18.4 / macOS 15.4 or newer. On older iPhones everything else works.

**Moving progress from the desktop or Android app:** in that app, **Settings > Export
progress**. On the website, **Settings > Import progress**, and choose the file. It replaces
the progress in your account.

---

## 6. Backups

Everything that matters is in `~/freesheet/web/data/` (progress, as `users/<id>/progress.json`
with a `.bak` copy of the previous save) and `~/freesheet/web/.env` (settings). Back up both,
for example to a USB stick:

```sh
cd ~/freesheet/web
tar czf ~/freesheet-backup-$(date +%F).tar.gz data .env
```

Then copy the `.tar.gz` file somewhere safe. To restore: stop FreeSheet
(`sudo systemctl stop freesheet-web`), unpack it in `~/freesheet/web` with
`tar xzf <file>`, and start it again (`sudo systemctl start freesheet-web`).

---

## Updates

FreeSheet **updates itself**. Once an hour it checks GitHub. When there is a new version, it
waits until nobody has had FreeSheet open for 15 minutes, then installs it and restarts.
Settings in `web/.env`:

- `AUTO_UPDATE=0` turns this off;
- `UPDATE_CHECK_MINUTES` and `UPDATE_IDLE_MINUTES` change the timing.

To update by hand: `cd ~/freesheet/web && ./update.sh`.

## Day-to-day commands

```sh
systemctl status freesheet-web          # is it running?
journalctl -u freesheet-web -f          # its log (Ctrl+C to stop watching)
sudo systemctl restart freesheet-web    # restart (after editing .env)
./uninstall.sh                          # remove the service (keeps your data unless you say so)
```

The laptop has to stay **on and awake**: in **Power Manager**, set "When laptop lid is
closed" to **Switch off display** and turn off sleep when plugged in.

---

## If port forwarding stops working: Cloudflare Tunnel

Some internet providers put homes behind **CGNAT** (you share a public IP with neighbours),
and then port forwarding can't work. A **Cloudflare Tunnel** works anyway, because the
laptop connects *out* to Cloudflare. It is free, but you need your own domain name added to
Cloudflare (from about R150 a year).

1. Install `cloudflared` and log in: follow
   <https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/get-started/create-local-tunnel/>.
2. Create a tunnel that sends `https://freesheet.<your-domain>` to `http://localhost:5000`.
3. In `web/.env`: empty `HTTPS_CERT` and `HTTPS_KEY` (Cloudflare does the HTTPS), and add
   `COOKIE_SECURE=1` and `TRUST_PROXY=1`. Restart FreeSheet.
4. In Google Cloud, add `https://freesheet.<your-domain>` as an Authorised JavaScript origin.

## Docker (optional)

If you prefer Docker to `install.sh`:

```sh
cd ~/freesheet/web
cp .env.example .env && nano .env      # fill in GOOGLE_CLIENT_ID and ALLOWED_EMAILS
docker compose up -d --build
```

Progress is kept in the `freesheet-data` Docker volume. For HTTPS, put `fullchain.pem` and
`key.pem` in `web/certs/` and set `HTTPS_CERT=/certs/fullchain.pem` and
`HTTPS_KEY=/certs/key.pem` in `.env`. Automatic updates are off in Docker. To update:
`git pull && docker compose up -d --build`.
