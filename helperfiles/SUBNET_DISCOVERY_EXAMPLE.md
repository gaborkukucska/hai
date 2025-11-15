# HAI-Net Automatic Subnet Discovery

## How It Works

The enhanced discovery system automatically finds and scans all network subnets your system has access to.

### Example Network Setup

```
Your Machine: 192.168.1.100
├── Interface eth0: 192.168.1.100/24 (Router A - main network)
├── Interface eth1: 192.168.2.150/24 (Router B - secondary network)  
└── Interface wlan0: 10.0.0.50/24 (WiFi network)

Remote LLM APIs:
├── 192.168.1.200:11434 (Ollama on Router A)
├── 192.168.2.75:11434 (Ollama on Router B) ← Previously not discoverable!
└── 10.0.0.120:11434 (Ollama on WiFi)
```

### Discovery Process

1. **Startup**: HAI-Net initializes and runs `ip addr show`
2. **Parse interfaces**: Extracts all subnets:
   - 192.168.1.0/24
   - 192.168.2.0/24
   - 10.0.0.0/24
3. **Scan each subnet**: Probes common IPs (1, 10, 20, ..., 250) on ports:
   - 11434 (Ollama)
   - 8000 (vLLM)
   - 4000 (LiteLLM)
   - 8080 (OpenAI-compatible)
4. **Build catalog**: All discovered models from all subnets

### Expected Log Output

```
[INFO] Starting provider discovery scan
[INFO] Scanning localhost for AI providers
[INFO] ✓ Found Ollama at http://localhost:11434 (5 models)
[INFO] Auto-discovering accessible subnets...
[INFO] Discovered subnet: 192.168.1.0/24
[INFO] Discovered subnet: 192.168.2.0/24
[INFO] Discovered subnet: 10.0.0.0/24
[INFO] Scanning subnet 192.168.1.0/24 for AI providers (common ports)
[INFO] ✓ Found Ollama at http://192.168.1.200:11434 (8 models)
[INFO] Scanning subnet 192.168.2.0/24 for AI providers (common ports)
[INFO] ✓ Found Ollama at http://192.168.2.75:11434 (12 models)
[INFO] Scanning subnet 10.0.0.0/24 for AI providers (common ports)
[INFO] ✓ Found Ollama at http://10.0.0.120:11434 (6 models)
[INFO] Discovery complete: 4 providers found
[INFO] Total models in catalog: 31 models
```

## Performance Optimization

- **Quick probes**: 500ms timeout per endpoint
- **Selective scanning**: Only scans every 10th IP (26 IPs per subnet instead of 254)
- **Parallel scanning**: Async/await for concurrent checks
- **Smart filtering**: Skips loopback (127.x) and embedding models

## Manual Override (Optional)

If you need to scan additional subnets not visible via `ip addr`:

```bash
export HAINET_EXTRA_SUBNETS="172.16.0.0/24,192.168.100.0/24"
```

## No Configuration Required

The system **just works** out of the box. No manual IP entry needed!
