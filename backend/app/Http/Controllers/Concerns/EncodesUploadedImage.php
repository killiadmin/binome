<?php

namespace App\Http\Controllers\Concerns;

use Illuminate\Http\UploadedFile;

trait EncodesUploadedImage
{
    /**
     * Encode un fichier uploadé en data URI base64, pour stockage direct en base de données.
     */
    private function encodeImage(?UploadedFile $file): ?string
    {
        if (!$file) {
            return null;
        }

        $data = base64_encode(file_get_contents($file->getRealPath()));

        return "data:{$file->getMimeType()};base64,{$data}";
    }
}
