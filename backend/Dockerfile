FROM php:8.2-cli

RUN apt-get update && apt-get install -y --no-install-recommends \
        git unzip libzip-dev libonig-dev libpng-dev libxml2-dev \
    && docker-php-ext-install pdo_mysql mbstring bcmath zip pcntl \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

COPY --from=composer:2 /usr/bin/composer /usr/bin/composer

WORKDIR /var/www/html

COPY composer.json composer.lock ./
RUN composer install --no-scripts --no-interaction --prefer-dist --no-autoloader

COPY . .

RUN composer dump-autoload --optimize

EXPOSE 8000 8080

CMD ["php", "artisan", "serve", "--host=0.0.0.0", "--port=8000"]
