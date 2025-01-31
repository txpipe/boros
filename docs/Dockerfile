FROM node:22-alpine AS dependencies-env
RUN npm i -g pnpm
COPY . /app

FROM dependencies-env AS development-dependencies-env
COPY ./package.json ./pnpm-lock.yaml /app/
WORKDIR /app
RUN pnpm i --frozen-lockfile

FROM dependencies-env AS production-dependencies-env
COPY ./package.json ./pnpm-lock.yaml /app/
WORKDIR /app
RUN pnpm i --prod --frozen-lockfile --ignore-scripts

FROM dependencies-env AS build-env
COPY ./package.json ./pnpm-lock.yaml /app/
COPY --from=development-dependencies-env /app/node_modules /app/node_modules
WORKDIR /app
RUN pnpm build

FROM dependencies-env
COPY ./package.json ./pnpm-lock.yaml /app/
COPY --from=production-dependencies-env /app/node_modules /app/node_modules
COPY --from=build-env /app/.next /app/.next
WORKDIR /app
EXPOSE 3000
CMD ["pnpm", "start"]