FROM node:22-slim

WORKDIR /app

# Copy package files
COPY package.json package-lock.json ./

# Install dependencies
RUN npm ci --no-audit --no-fund

# Copy source
COPY . .

# Build TypeScript
RUN npm run build

# Expose metrics/health port
EXPOSE 9464

# Start relayer
CMD ["npm", "start"]
