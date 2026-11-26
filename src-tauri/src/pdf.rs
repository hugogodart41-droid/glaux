//! Carrousel LinkedIn : un PDF dont chaque page est une image JPEG, sans bibliothèque.
//!
//! LinkedIn affiche un PDF comme un carrousel (une page = une diapositive). L'interface
//! dessine les pages (couverture, un indicateur par page, sources) en JPEG ; ce module les
//! assemble dans un PDF minimal : chaque page contient une seule image, à pleine taille.
//!
//! Structure du fichier (objets numérotés) :
//! 1 catalogue → 2 liste des pages ; 3 informations (titre, auteur) ;
//! pour chaque page : page, contenu (« dessine l'image »), image (JPEG tel quel, DCTDecode).

/// Pages au plus dans un carrousel (LinkedIn en accepte 300 ; le panneau en compte ~20).
pub const MAX_PAGES: usize = 40;

/// Taille d'une image JPEG (largeur, hauteur), lue dans son en-tête (marqueur SOF).
/// None si ce n'est pas un JPEG.
pub fn jpeg_size(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        // Octets de remplissage (0xFF répétés) : on avance.
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([data[i + 2], data[i + 3]]));
        // SOF0 à SOF15 (sauf DHT C4, JPG C8, DAC CC) : hauteur puis largeur.
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let height = u16::from_be_bytes([data[i + 5], data[i + 6]]);
            let width = u16::from_be_bytes([data[i + 7], data[i + 8]]);
            return (width > 0 && height > 0).then_some((u32::from(width), u32::from(height)));
        }
        if length < 2 {
            return None;
        }
        i += 2 + length;
    }
    None
}

/// Texte PDF quelconque (accents compris) : UTF-16 en hexadécimal, « <FEFF…> ».
fn pdf_text(text: &str) -> String {
    let hex: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
    format!("<FEFF{hex}>")
}

/// Construit le PDF : une page par image JPEG, à la taille de l'image (96 points par pouce,
/// soit 810 × 1012,5 points pour une page 1080 × 1350). Refuse une page qui n'est pas un JPEG.
pub fn build(pages: &[&[u8]], title: &str, author: &str) -> Result<Vec<u8>, String> {
    if pages.is_empty() || pages.len() > MAX_PAGES {
        return Err(format!("entre 1 et {MAX_PAGES} pages attendues"));
    }
    let sizes = pages
        .iter()
        .enumerate()
        .map(|(k, page)| {
            jpeg_size(page).ok_or_else(|| format!("page {} : image JPEG illisible", k + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut pdf: Vec<u8> = Vec::new();
    // Ligne d'en-tête, puis 4 octets > 127 : signale aux outils un fichier binaire.
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets: Vec<usize> = Vec::new(); // position de chaque objet, pour la table xref

    let page_ids: Vec<usize> = (0..pages.len()).map(|k| 4 + 3 * k).collect();
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();

    let mut object = |pdf: &mut Vec<u8>, body: &[u8]| {
        offsets.push(pdf.len());
        let id = offsets.len();
        pdf.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    };

    object(&mut pdf, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(
        &mut pdf,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    object(
        &mut pdf,
        format!(
            "<< /Title {} /Author {} /Producer (Glaux) >>",
            pdf_text(title),
            pdf_text(author)
        )
        .as_bytes(),
    );

    for (k, (page, (width, height))) in pages.iter().zip(&sizes).enumerate() {
        let (page_id, content_id, image_id) = (4 + 3 * k, 5 + 3 * k, 6 + 3 * k);
        // Taille de la page en points (1 px = 0,75 pt).
        let (w, h) = (f64::from(*width) * 0.75, f64::from(*height) * 0.75);
        object(
            &mut pdf,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
                 /Resources << /XObject << /Im{page_id} {image_id} 0 R >> >> \
                 /Contents {content_id} 0 R >>"
            )
            .as_bytes(),
        );
        let draw = format!("q {w} 0 0 {h} 0 0 cm /Im{page_id} Do Q");
        object(
            &mut pdf,
            format!("<< /Length {} >>\nstream\n{draw}\nendstream", draw.len()).as_bytes(),
        );
        let mut image = format!(
            "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            page.len()
        )
        .into_bytes();
        image.extend_from_slice(page);
        image.extend_from_slice(b"\nendstream");
        object(&mut pdf, &image);
    }

    // Table des positions (20 octets par ligne), puis la fin du fichier.
    let xref = pdf.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for offset in &offsets {
        table.push_str(&format!("{offset:010} 00000 n \n"));
    }
    table.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        offsets.len() + 1
    ));
    pdf.extend_from_slice(table.as_bytes());
    Ok(pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En-tête JPEG minimal : début d'image, segment APP0, SOF0 avec la taille, fin d'image.
    fn fake_jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        jpeg.extend_from_slice(&height.to_be_bytes());
        jpeg.extend_from_slice(&width.to_be_bytes());
        jpeg.extend_from_slice(&[0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    #[test]
    fn lit_la_taille_d_un_jpeg() {
        assert_eq!(jpeg_size(&fake_jpeg(1080, 1350)), Some((1080, 1350)));
        assert_eq!(jpeg_size(b"\x89PNG\r\n\x1a\n"), None);
        assert_eq!(jpeg_size(&[0xFF, 0xD8, 0xFF]), None); // tronqué
    }

    #[test]
    fn pdf_avec_une_page_par_image() {
        let pages = [
            fake_jpeg(1080, 1350),
            fake_jpeg(1080, 1350),
            fake_jpeg(1080, 1350),
        ];
        let refs: Vec<&[u8]> = pages.iter().map(Vec::as_slice).collect();
        let pdf = build(&refs, "Glaux — la zone euro en chiffres", "Glaux").unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(text.ends_with("%%EOF\n"));
        assert!(text.contains("/Count 3"));
        assert!(text.contains("/MediaBox [0 0 810 1012.5]"));
        assert_eq!(text.matches("/Subtype /Image").count(), 3);

        // Chaque entrée de la table xref pointe bien sur « n 0 obj » (positions en octets).
        let marker = b"startxref\n";
        let at = pdf
            .windows(marker.len())
            .rposition(|w| w == marker)
            .unwrap();
        let tail = std::str::from_utf8(&pdf[at + marker.len()..]).unwrap();
        let start: usize = tail.lines().next().unwrap().parse().unwrap();
        let table = std::str::from_utf8(&pdf[start..]).unwrap();
        assert!(table.starts_with("xref\n0 13\n"));
        let entries: Vec<usize> = table
            .lines()
            .skip(3)
            .take(12)
            .map(|line| line[..10].parse().unwrap())
            .collect();
        for (k, offset) in entries.iter().enumerate() {
            assert!(
                pdf[*offset..].starts_with(format!("{} 0 obj", k + 1).as_bytes()),
                "objet {} mal placé",
                k + 1
            );
        }
        // Titre accentué encodé en UTF-16.
        assert!(text.contains("/Title <FEFF0047006C006100750078"));
    }

    #[test]
    fn refuse_ce_qui_n_est_pas_un_jpeg() {
        assert!(build(&[], "t", "a").is_err());
        assert!(build(&[b"pas une image".as_slice()], "t", "a").is_err());
        let page = fake_jpeg(10, 10);
        let many = vec![page.as_slice(); MAX_PAGES + 1];
        assert!(build(&many, "t", "a").is_err());
    }
}
