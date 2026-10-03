# Cloudflared Tunnel Setup

## Prerequisites
- Cloudflare account with a domain
- Cloudflared CLI installed

## 1. Create Tunnel
```bash
cloudflared tunnel create suban
```

## 2. Get Tunnel Token
Go to Cloudflare Dashboard → Networks → Tunnels → Create a tunnel → Copy the token.

## 3. Configure
Edit `config.yml` and replace `<YOUR_TUNNEL_ID>` with your tunnel ID.

## 4. Run with Docker
```bash
docker run -d --name cloudflared \
  --network docker_suban-net \
  -v ./cloudflared/credentials.json:/etc/cloudflared/credentials.json:ro \
  -v ./cloudflared/config.yml:/etc/cloudflared/config.yml:ro \
  cloudflared:latest tunnel run
```

## 5. DNS Routes
In Cloudflare dashboard, add CNAME records:
- `api.suban.org` → `<tunnel-id>.cfargotunnel.com`
- `oracle.suban.org` → `<tunnel-id>.cfargotunnel.com`
- `rpc.suban.org` → `<tunnel-id>.cfargotunnel.com`
- `horizon.suban.org` → `<tunnel-id>.cfargotunnel.com`

## Environment Variables
```bash
TUNNEL_TOKEN=<your-cloudflare-tunnel-token>
```
