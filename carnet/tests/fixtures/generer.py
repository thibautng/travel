# -*- coding: utf-8 -*-
"""Fabrique les fixtures de test des contraintes C1 a C10.

Chaque fixture est un JPEG minimal : marqueur de debut, bloc EXIF fabrique,
marqueur de fin. Quelques centaines d'octets au lieu de plusieurs megaoctets,
pour des fichiers dont seules les metadonnees sont le sujet du test.

Ces fichiers ne sont PAS des images decodables : le lot 1 ne lit que l'EXIF.
Le lot 3, qui genere des derives, aura besoin de ses propres fixtures.

Chaque cas documente le fichier reel dont il est tire.

Usage : python generer.py
"""
from __future__ import print_function
import io
import os
import struct

ICI = os.path.dirname(os.path.abspath(__file__))

# Types EXIF
ASCII, SHORT, LONG, RATIONAL, BYTE = 2, 3, 4, 5, 1

T_MAKE, T_MODEL, T_ORIENTATION = 0x010F, 0x0110, 0x0112
T_EXIF_IFD, T_GPS_IFD = 0x8769, 0x8825
T_DTO, T_OFFSET_DTO = 0x9003, 0x9011
T_LARGEUR, T_HAUTEUR = 0xA002, 0xA003
G_LATREF, G_LAT, G_LONREF, G_LON, G_ALTREF, G_ALT = 0x01, 0x02, 0x03, 0x04, 0x05, 0x06


def dms(degres_decimaux):
    """Convertit des degres decimaux en trois rationnels degre/minute/seconde."""
    v = abs(degres_decimaux)
    d = int(v)
    minutes = (v - d) * 60
    m = int(minutes)
    # `minutes` est deja exprime en minutes : la fraction restante se convertit
    # en secondes par 60, pas par 3600.
    s = (minutes - m) * 60
    return [(d, 1), (m, 1), (int(round(s * 100)), 100)]


class Ifd(object):
    def __init__(self):
        self.entrees = []

    def ajouter(self, tag, typ, valeurs):
        self.entrees.append((tag, typ, valeurs))

    def taille(self):
        return 2 + 12 * len(self.entrees) + 4


def encoder_valeur(typ, valeurs):
    if typ == ASCII:
        brut = valeurs.encode('ascii') + b'\x00'
        return len(brut), brut
    if typ == SHORT:
        return len(valeurs), b''.join(struct.pack('<H', v) for v in valeurs)
    if typ == LONG:
        return len(valeurs), b''.join(struct.pack('<I', v) for v in valeurs)
    if typ == RATIONAL:
        return len(valeurs), b''.join(struct.pack('<II', n, d) for n, d in valeurs)
    if typ == BYTE:
        return len(valeurs), bytes(bytearray(valeurs))
    raise ValueError('type inconnu')


def construire_exif(ifd0, ifd_exif, ifd_gps):
    """Assemble le bloc TIFF : entete, trois IFD, puis les donnees longues."""
    entete = b'II' + struct.pack('<HI', 42, 8)
    off_ifd0 = 8
    off_exif = off_ifd0 + ifd0.taille()
    off_gps = off_exif + (ifd_exif.taille() if ifd_exif.entrees else 0)
    base_donnees = off_gps + (ifd_gps.taille() if ifd_gps.entrees else 0)

    if ifd_exif.entrees:
        ifd0.ajouter(T_EXIF_IFD, LONG, [off_exif])
    if ifd_gps.entrees:
        ifd0.ajouter(T_GPS_IFD, LONG, [off_gps])

    # Les offsets des sous-IFD dependent de la taille d'IFD0, qui vient de
    # changer : on recalcule tout une fois les pointeurs ajoutes.
    off_exif = off_ifd0 + ifd0.taille()
    off_gps = off_exif + (ifd_exif.taille() if ifd_exif.entrees else 0)
    base_donnees = off_gps + (ifd_gps.taille() if ifd_gps.entrees else 0)
    for i, (tag, typ, valeurs) in enumerate(ifd0.entrees):
        if tag == T_EXIF_IFD:
            ifd0.entrees[i] = (tag, typ, [off_exif])
        elif tag == T_GPS_IFD:
            ifd0.entrees[i] = (tag, typ, [off_gps])

    donnees = bytearray()

    def serialiser(ifd):
        corps = struct.pack('<H', len(ifd.entrees))
        for tag, typ, valeurs in sorted(ifd.entrees, key=lambda e: e[0]):
            compte, brut = encoder_valeur(typ, valeurs)
            if len(brut) <= 4:
                brut = brut + b'\x00' * (4 - len(brut))
                corps += struct.pack('<HHI', tag, typ, compte) + brut
            else:
                offset = base_donnees + len(donnees)
                donnees.extend(brut)
                if len(donnees) % 2:
                    donnees.append(0)
                corps += struct.pack('<HHII', tag, typ, compte, offset)
        return corps + struct.pack('<I', 0)

    corps0 = serialiser(ifd0)
    corps_exif = serialiser(ifd_exif) if ifd_exif.entrees else b''
    corps_gps = serialiser(ifd_gps) if ifd_gps.entrees else b''
    return entete + corps0 + corps_exif + corps_gps + bytes(donnees)


def ecrire_jpeg(chemin, bloc_exif, note):
    """SOI, APP1 Exif, commentaire, EOI."""
    sortie = bytearray(b'\xff\xd8')
    if bloc_exif is not None:
        charge = b'Exif\x00\x00' + bloc_exif
        sortie += b'\xff\xe1' + struct.pack('>H', len(charge) + 2) + charge
    commentaire = note.encode('utf-8')
    sortie += b'\xff\xfe' + struct.pack('>H', len(commentaire) + 2) + commentaire
    sortie += b'\xff\xd9'
    dossier = os.path.dirname(chemin)
    if dossier and not os.path.isdir(dossier):
        os.makedirs(dossier)
    with io.open(chemin, 'wb') as f:
        f.write(bytes(sortie))
    return len(sortie)


def fabriquer(nom, note, date=None, offset=None, position=None,
              appareil=('OPPO', 'OPPO Reno6 Pro 5G'), taille=(4096, 3072)):
    """position : (lat, lon, alt) en degres decimaux signes, alt en metres."""
    ifd0, ifd_exif, ifd_gps = Ifd(), Ifd(), Ifd()
    if appareil:
        ifd0.ajouter(T_MAKE, ASCII, appareil[0])
        ifd0.ajouter(T_MODEL, ASCII, appareil[1])
        ifd0.ajouter(T_ORIENTATION, SHORT, [1])
    if date:
        ifd_exif.ajouter(T_DTO, ASCII, date)
    if offset:
        ifd_exif.ajouter(T_OFFSET_DTO, ASCII, offset)
    if taille:
        ifd_exif.ajouter(T_LARGEUR, LONG, [taille[0]])
        ifd_exif.ajouter(T_HAUTEUR, LONG, [taille[1]])
    if position:
        lat, lon, alt = position
        ifd_gps.ajouter(G_LATREF, ASCII, 'N' if lat >= 0 else 'S')
        ifd_gps.ajouter(G_LAT, RATIONAL, dms(lat))
        ifd_gps.ajouter(G_LONREF, ASCII, 'E' if lon >= 0 else 'W')
        ifd_gps.ajouter(G_LON, RATIONAL, dms(lon))
        if alt is not None:
            ifd_gps.ajouter(G_ALTREF, BYTE, [0 if alt >= 0 else 1])
            ifd_gps.ajouter(G_ALT, RATIONAL, [(int(round(abs(alt) * 100)), 100)])
    bloc = construire_exif(ifd0, ifd_exif, ifd_gps)
    octets = ecrire_jpeg(os.path.join(ICI, nom), bloc, note)
    print('  %-34s %4d octets  %s' % (nom, octets, note))


def main():
    print('Fixtures des contraintes C1 a C10 :')

    # C1 : altitude reelle, position satellite. D'apres IMG20260728123627.jpg,
    # Hollentalangerhutte, 1428 m.
    fabriquer('c01_altitude_reelle.jpg', 'C1 position fiable',
              date='2026:07:28 12:36:27', offset='+02:00',
              position=(47.42180, 11.03670, 1428.0))

    # C1 : meme journee, altitude nulle, donc position reseau.
    fabriquer('c01_altitude_nulle.jpg', 'C1 position suspecte',
              date='2026:07:28 18:16:04', offset='+02:00',
              position=(47.44100, 11.02500, 0.0))

    # C2 : deux medias a la meme position, a plus de vingt minutes.
    # D'apres le groupe du 27 juillet, dix-huit photos au meme point.
    for suffixe, heure in (('matin', '11:50:12'), ('soir', '18:16:44')):
        fabriquer('c02_clone_%s.jpg' % suffixe, 'C2 position clonee',
                  date='2026:07:29 %s' % heure, offset='+02:00',
                  position=(47.45753, 10.98037, 0.0))

    # C3 : le nom porte la date du partage, l'EXIF la vraie date.
    # Fichier reel : IMG_20260730_071148.jpg, prise le 28 juillet.
    fabriquer('IMG_20260730_071148.jpg', 'C3 nom menteur',
              date='2026:07:28 12:36:27', offset='+02:00',
              position=(47.42180, 11.03670, 1428.0))

    # C4 : GoPro HERO7, aucune position, horloge perdue au 3 janvier 2016.
    fabriquer('GOPR2699.JPG', 'C4 horloge perdue',
              date='2016:01:03 19:02:45', appareil=('GoPro', 'HERO7 Black'))

    # C5 : deux positions fiables de la meme journee, eloignees dans le temps
    # et dans l'espace. D'apres le 6 aout, Tassenbach vers Lienz.
    fabriquer('c05_trou_avant.jpg', 'C5 trou candidat, avant',
              date='2026:08:06 13:30:42', offset='+02:00',
              position=(46.73300, 12.28000, 1100.0))
    fabriquer('c05_trou_apres.jpg', 'C5 trou candidat, apres',
              date='2026:08:06 18:59:11', offset='+02:00',
              position=(46.82900, 12.76900, 673.0))

    # C6 : nom hostile. Le tilde devient un tiret, la variante est conservee.
    fabriquer('IMG20260808113008~2.jpg', 'C6 nom normalise',
              date='2026:08:08 11:30:08', offset='+02:00',
              position=(46.61243, 12.29368, 2331.958))

    # C8 : homonyme place dans un sous-dossier. Ignore par defaut via
    # dossiers_ignores, et cause de collision quand on ne l'ignore pas.
    fabriquer('c08_homonyme.jpg', 'C8 fichier de la racine',
              date='2026:08:09 10:00:00', offset='+02:00',
              position=(46.50000, 12.00000, 900.0))
    fabriquer(os.path.join('[Originals]', 'c08_homonyme.jpg'),
              'C8 original homonyme',
              date='2026:08:09 10:00:00', offset='+02:00',
              position=(46.50000, 12.00000, 900.0))

    # C9 : hemisphere sud et ouest. D'apres les photos de Polynesie, dont les
    # references S et W, ignorees, placeraient Tahiti dans le Pacifique nord.
    fabriquer('c09_hemisphere_sud_ouest.jpg', 'C9 references S et W',
              date='2026:08:10 12:00:00', offset='+02:00',
              position=(-17.53924, -149.56765, 14.836),
              appareil=('HMD Global', 'Nokia 7 plus'))

    # C10 : recu par messagerie, aucun bloc EXIF. La date vient du nom.
    ecrire_jpeg(os.path.join(ICI, 'IMG-20260811-WA0000.jpg'), None,
                'C10 aucun EXIF, date lue dans le nom')
    print('  %-34s            C10 aucun EXIF' % 'IMG-20260811-WA0000.jpg')


if __name__ == '__main__':
    main()
