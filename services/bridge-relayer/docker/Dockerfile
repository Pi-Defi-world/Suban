FROM node:20-slim

WORKDIR /app

# Copy package files
COPY package.json ./

# Install dependencies
RUN npm install

# Copy source
COPY . .

# Build TypeScript
RUN npm run build

# Expose port (for health checks)
EXPOSE 3003

# Start relayer
CMD ["npm", "start"]
